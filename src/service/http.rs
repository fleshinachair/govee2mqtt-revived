// Every handler here returns `Result<Response, Response>`, the idiomatic axum
// shape: the error arm is an already-rendered HTTP response, not an error value
// waiting to be formatted. `result_large_err` measures that arm against a
// 128-byte budget and flags all 13 handlers. Boxing an `axum::response::Response`
// to satisfy it would add an allocation and a deref to every request path while
// changing nothing a user can observe, so the lint is allowed for this module.
//
// The lint began firing when CI's `dtolnay/rust-toolchain@stable` rolled from
// 1.94 to 1.98; the code it flags is unchanged and predates that.
#![allow(clippy::result_large_err)]

use crate::service::coordinator::Coordinator;
use crate::service::device::{Device, DeviceState};
use crate::service::diagnostics;
use crate::service::state::StateHandle;
use anyhow::Context;
use axum::extract::{ConnectInfo, Request};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use std::net::Ipv4Addr;
use std::net::{IpAddr, SocketAddr};
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

fn response_with_code<T: ToString + std::fmt::Display>(code: StatusCode, err: T) -> Response {
    if !code.is_success() {
        log::error!("err: {err:#}");
    }

    let mut response = Json(serde_json::json!({
        "code": code.as_u16(),
        "msg": format!("{err:#}")
    }))
    .into_response();
    *response.status_mut() = code;
    response
}

fn generic<T: ToString + std::fmt::Display>(err: T) -> Response {
    response_with_code(StatusCode::INTERNAL_SERVER_ERROR, err)
}

fn not_found<T: ToString + std::fmt::Display>(err: T) -> Response {
    response_with_code(StatusCode::NOT_FOUND, err)
}

fn bad_request<T: ToString + std::fmt::Display>(err: T) -> Response {
    response_with_code(StatusCode::BAD_REQUEST, err)
}

async fn resolve_device_for_control(
    state: &StateHandle,
    id: &str,
) -> Result<Coordinator, Response> {
    state
        .resolve_device_for_control(id)
        .await
        .map_err(not_found)
}

async fn resolve_device_read_only(state: &StateHandle, id: &str) -> Result<Device, Response> {
    state.resolve_device_read_only(id).await.map_err(not_found)
}

/// Returns a json array of device information
async fn list_devices(State(state): State<StateHandle>) -> Result<Response, Response> {
    let mut devices = state.devices().await;
    devices.sort_by_key(|d| (d.room_name().map(|name| name.to_string()), d.name()));
    let mqtt_connected = state.get_hass_client().await.is_some();

    #[derive(Serialize)]
    struct DeviceItem {
        pub sku: String,
        pub id: String,
        pub name: String,
        pub room: Option<String>,
        pub ip: Option<IpAddr>,
        pub state: Option<DeviceState>,
        pub mqtt_connected: bool,
        pub api_available: bool,
        pub lan_active: bool,
        pub cloud_online: Option<bool>,
    }

    let devices: Vec<_> = devices
        .into_iter()
        .map(|d| {
            let device_state = d.device_state();
            let cloud_online = device_state.as_ref().and_then(|state| state.online);
            let api_available = d.http_device_info.is_some();
            let ip = d.ip_addr();
            DeviceItem {
                name: d.name(),
                room: d.room_name().map(|r| r.to_string()),
                ip,
                state: device_state,
                mqtt_connected,
                api_available,
                lan_active: ip.is_some(),
                cloud_online,
                sku: d.sku,
                id: d.id,
            }
        })
        .collect();

    Ok(Json(devices).into_response())
}

/// Turns on a given device
async fn device_power_on(
    State(state): State<StateHandle>,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let device = resolve_device_for_control(&state, &id).await?;

    state
        .device_power_on(&device, true)
        .await
        .map_err(generic)?;

    Ok(response_with_code(StatusCode::OK, "ok"))
}

/// Turns off a given device
async fn device_power_off(
    State(state): State<StateHandle>,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let device = resolve_device_for_control(&state, &id).await?;

    state
        .device_power_on(&device, false)
        .await
        .map_err(generic)?;

    Ok(response_with_code(StatusCode::OK, "ok"))
}

/// Sets the brightness level of a given device
async fn device_set_brightness(
    State(state): State<StateHandle>,
    Path((id, level)): Path<(String, u8)>,
) -> Result<Response, Response> {
    let device = resolve_device_for_control(&state, &id).await?;

    state
        .device_set_brightness(&device, level)
        .await
        .map_err(generic)?;

    Ok(response_with_code(StatusCode::OK, "ok"))
}

/// Sets the color temperature of a given device
async fn device_set_color_temperature(
    State(state): State<StateHandle>,
    Path((id, kelvin)): Path<(String, u32)>,
) -> Result<Response, Response> {
    let device = resolve_device_for_control(&state, &id).await?;

    state
        .device_set_color_temperature(&device, kelvin)
        .await
        .map_err(generic)?;

    Ok(response_with_code(StatusCode::OK, "ok"))
}

/// Sets the RGB color of a given device
async fn device_set_color(
    State(state): State<StateHandle>,
    Path((id, color)): Path<(String, String)>,
) -> Result<Response, Response> {
    let color = csscolorparser::parse(&color)
        .map_err(|err| bad_request(format!("error parsing color '{color}': {err}")))?;
    let [r, g, b, _a] = color.to_rgba8();

    let device = resolve_device_for_control(&state, &id).await?;

    state
        .device_set_color_rgb(&device, r, g, b)
        .await
        .map_err(generic)?;

    Ok(response_with_code(StatusCode::OK, "ok"))
}

/// Activates the named scene for a given device
async fn device_set_scene(
    State(state): State<StateHandle>,
    Path((id, scene)): Path<(String, String)>,
) -> Result<Response, Response> {
    let device = resolve_device_for_control(&state, &id).await?;

    state
        .device_set_scene(&device, &scene)
        .await
        .map_err(generic)?;

    Ok(response_with_code(StatusCode::OK, "ok"))
}

/// Returns a JSON array of the available scenes with category structure preserved
async fn device_list_scenes_categorized(
    State(state): State<StateHandle>,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let device = resolve_device_read_only(&state, &id).await?;

    let catalog = state
        .device_list_scenes_categorized(&device)
        .await
        .map_err(generic)?;

    Ok(Json(catalog).into_response())
}

/// Returns a JSON array of the available scene names for a given device
async fn device_list_scenes(
    State(state): State<StateHandle>,
    Path(id): Path<String>,
) -> Result<Response, Response> {
    let device = resolve_device_read_only(&state, &id).await?;

    let scenes = state.device_list_scenes(&device).await.map_err(generic)?;

    Ok(Json(scenes).into_response())
}

async fn list_one_clicks(State(state): State<StateHandle>) -> Result<Response, Response> {
    let undoc = state
        .get_undoc_client()
        .await
        .ok_or_else(|| {
            anyhow::anyhow!("Govee cloud API unavailable: is your email/password configured?")
        })
        .map_err(generic)?;
    let items = undoc.parse_one_clicks().await.map_err(generic)?;

    Ok(Json(items).into_response())
}

async fn activate_one_click(
    State(state): State<StateHandle>,
    Path(name): Path<String>,
) -> Result<Response, Response> {
    let undoc = state
        .get_undoc_client()
        .await
        .ok_or_else(|| {
            anyhow::anyhow!("Govee cloud API unavailable: is your email/password configured?")
        })
        .map_err(generic)?;
    let items = undoc.parse_one_clicks().await.map_err(generic)?;
    let item = items
        .iter()
        .find(|item| item.name == name)
        .ok_or_else(|| anyhow::anyhow!("didn't find item {name}"))
        .map_err(not_found)?;

    let iot = state
        .get_iot_client()
        .await
        .ok_or_else(|| anyhow::anyhow!("AWS IoT client is not available"))
        .map_err(generic)?;

    iot.activate_one_click(item).await.map_err(generic)?;

    Ok(response_with_code(StatusCode::OK, "ok"))
}

/// The sanitized report shown on the "Report a problem" page
async fn diagnostics_report(State(state): State<StateHandle>) -> Response {
    Json(diagnostics::collect(&state).await).into_response()
}

async fn redirect_to_index() -> Response {
    axum::response::Redirect::to("assets/index.html").into_response()
}

fn build_router(state: StateHandle) -> Router {
    Router::new()
        .route("/api/devices", get(list_devices))
        .route("/api/diagnostics", get(diagnostics_report))
        .route("/api/device/{id}/power/on", get(device_power_on))
        .route("/api/device/{id}/power/off", get(device_power_off))
        .route(
            "/api/device/{id}/brightness/{level}",
            get(device_set_brightness),
        )
        .route(
            "/api/device/{id}/colortemp/{kelvin}",
            get(device_set_color_temperature),
        )
        .route("/api/device/{id}/color/{color}", get(device_set_color))
        .route("/api/device/{id}/scene/{scene}", get(device_set_scene))
        .route("/api/device/{id}/scenes", get(device_list_scenes))
        .route(
            "/api/device/{id}/scene-catalog",
            get(device_list_scenes_categorized),
        )
        .route("/api/oneclicks", get(list_one_clicks))
        .route("/api/oneclick/activate/{scene}", get(activate_one_click))
        .route("/", get(redirect_to_index))
        .nest_service("/assets", ServeDir::new("assets"))
        .with_state(state)
}

#[cfg(test)]
#[test]
fn test_build_router() {
    // axum has a history of chaning the URL syntax across
    // semver bumps; while that is OK, the syntax changes
    // are not caught at compile time, so we need a runtime
    // check to verify that the syntax is still good.
    // This next line will panic if axum decides that
    // the syntax is bad.
    let _ = build_router(StateHandle::default());
}

/// Home Assistant's Supervisor makes every ingress request from this address
const SUPERVISOR_ADDR: IpAddr = IpAddr::V4(Ipv4Addr::new(172, 30, 32, 2));

fn is_supervisor(addr: IpAddr) -> bool {
    addr.to_canonical() == SUPERVISOR_ADDR
}

async fn only_supervisor(
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    if is_supervisor(peer.ip()) {
        next.run(request).await
    } else {
        log::warn!(
            "refused a request on the ingress port from {ip}",
            ip = peer.ip()
        );
        StatusCode::FORBIDDEN.into_response()
    }
}

/// Serves the web page to Home Assistant's ingress, which has already checked
/// who is asking. Anyone else is turned away, as the port is open to the
/// network when the add-on uses host networking.
async fn serve_ingress(state: StateHandle, listener: TcpListener) -> anyhow::Result<()> {
    let app = build_router(state).layer(middleware::from_fn(only_supervisor));
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .context("ingress server stopped")
}

pub async fn run_ingress_server(state: StateHandle, port: u16) -> anyhow::Result<()> {
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .with_context(|| format!("run_ingress_server: binding to port {port}"))?;
    log::info!("ingress server is on port {port}");
    serve_ingress(state, listener).await
}

pub async fn run_http_server(state: StateHandle, addr: IpAddr, port: u16) -> anyhow::Result<()> {
    let app = build_router(state);
    let sa = SocketAddr::new(addr, port);
    let listener = TcpListener::bind(sa)
        .await
        .with_context(|| format!("run_http_server: binding to port {port}"))?;
    let addr = listener.local_addr()?;
    log::info!("http server addr is {addr:?}");
    if let Err(err) = axum::serve(listener, app).await {
        log::error!("http server stopped: {err:#}");
    }

    Ok(())
}

#[cfg(test)]
mod ingress_test {
    use super::*;

    #[test]
    fn only_the_supervisor_may_use_the_ingress_port() {
        let ok = |addr: &str| is_supervisor(addr.parse().unwrap());
        assert!(ok("172.30.32.2"));
        assert!(ok("::ffff:172.30.32.2"));
        assert!(!ok("172.30.32.1"));
        assert!(!ok("192.168.1.59"));
        assert!(!ok("127.0.0.1"));
        assert!(!ok("::1"));
    }

    #[tokio::test]
    async fn others_are_refused_on_the_ingress_port() -> anyhow::Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let base = format!("http://{}", listener.local_addr()?);
        let state = std::sync::Arc::new(crate::service::state::State::new());
        tokio::spawn(serve_ingress(state, listener));

        for path in [
            "/",
            "/api/devices",
            "/assets/index.html",
            "/api/device/x/power/on",
        ] {
            let response = reqwest::get(format!("{base}{path}")).await?;
            assert_eq!(response.status().as_u16(), 403, "{path}");
        }
        Ok(())
    }

    /// Ingress serves the page below a path of its own, where a link that
    /// starts with "/" would leave the bridge
    #[test]
    fn the_web_page_uses_only_relative_links() {
        for (name, text) in [
            ("index.html", include_str!("../../assets/index.html")),
            ("report.html", include_str!("../../assets/report.html")),
            ("report.js", include_str!("../../assets/report.js")),
            (
                "devices.js",
                include_str!("../../assets/components/devices.js"),
            ),
        ] {
            for quote in ['"', '\'', '`'] {
                for root in ["/assets", "/api"] {
                    let absolute = format!("{quote}{root}");
                    assert!(!text.contains(&absolute), "{name} links to {absolute}");
                }
            }
        }
    }
}

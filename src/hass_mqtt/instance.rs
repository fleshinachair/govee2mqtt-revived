use crate::hass_mqtt::base::EntityConfig;
use crate::service::hass::{
    device_availability_topic_for_id, offline_unavailable_enabled, HassClient,
};
use crate::service::state::StateHandle;
use anyhow::Context;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;

#[async_trait]
pub trait EntityInstance: Send + Sync {
    async fn publish_config(&self, state: &StateHandle, client: &HassClient) -> anyhow::Result<()>;
    async fn notify_state(&self, client: &HassClient) -> anyhow::Result<()>;
}

pub async fn publish_entity_config<T: Serialize>(
    integration: &str,
    state: &StateHandle,
    client: &HassClient,
    base: &EntityConfig,
    config: &T,
) -> anyhow::Result<()> {
    // TODO: remember all published topics for future GC

    let disco = state.get_hass_disco_prefix().await;
    let topic = format!(
        "{disco}/{integration}/{unique_id}/config",
        unique_id = base.unique_id
    );

    if offline_unavailable_enabled() {
        let config = serde_json::to_value(config).context("serializing entity config")?;
        client
            .publish_obj(topic, with_device_availability(config, base))
            .await
    } else {
        client.publish_obj(topic, config).await
    }
}

/// Make a device's entity depend on both the bridge and the device being
/// available. The Status sensor and the "Request Platform API State" button
/// are left alone: they are how you see and recheck an offline device.
fn with_device_availability(mut config: Value, base: &EntityConfig) -> Value {
    let device_id = base
        .device
        .identifiers
        .first()
        .and_then(|identifier| identifier.strip_prefix("gv2mqtt-"));
    let always_available = base.unique_id.ends_with("-gv2mqtt-status")
        || base.unique_id.ends_with("-request-platform-data");

    if let (Some(id), false, Some(map)) = (device_id, always_available, config.as_object_mut()) {
        if let Some(bridge) = map.remove("availability_topic") {
            map.insert(
                "availability".to_string(),
                json!([
                    {"topic": bridge},
                    {"topic": device_availability_topic_for_id(id)},
                ]),
            );
            map.insert("availability_mode".to_string(), json!("all"));
        }
    }
    config
}

#[derive(Default, Clone)]
pub struct EntityList {
    entities: Vec<Arc<dyn EntityInstance + Send + Sync + 'static>>,
}

impl EntityList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add<E: EntityInstance + Send + Sync + 'static>(&mut self, e: E) {
        self.entities.push(Arc::new(e));
    }

    /// Append another list's entities. Used to merge a device's entities in
    /// only after that device's enumeration has fully succeeded, so a
    /// failure partway through doesn't leave a partial set behind.
    pub fn extend(&mut self, other: EntityList) {
        self.entities.extend(other.entities);
    }

    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub async fn publish_config(
        &self,
        state: &StateHandle,
        client: &HassClient,
    ) -> anyhow::Result<()> {
        // Allow HASS time to process each entity before registering the next
        let delay = tokio::time::Duration::from_millis(100);
        for e in &self.entities {
            e.publish_config(state, client)
                .await
                .context("EntityList::publish_config")?;
            tokio::time::sleep(delay).await;
        }
        Ok(())
    }

    pub async fn notify_state(&self, client: &HassClient) -> anyhow::Result<()> {
        for e in &self.entities {
            e.notify_state(client)
                .await
                .context("EntityList::notify_state")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::hass_mqtt::base::Device;

    fn entity(identifier: &str, unique_id: &str) -> (Value, EntityConfig) {
        let base = EntityConfig {
            availability_topic: "gv2mqtt/availability".to_string(),
            unique_id: unique_id.to_string(),
            device: Device {
                identifiers: vec![identifier.to_string()],
                ..Default::default()
            },
            ..Default::default()
        };
        (serde_json::to_value(&base).unwrap(), base)
    }

    #[test]
    fn device_entities_also_depend_on_the_device() {
        let (config, base) = entity("gv2mqtt-AABB", "gv2mqtt-AABB");
        let config = with_device_availability(config, &base);

        assert!(config.get("availability_topic").is_none());
        assert_eq!(config["availability_mode"], "all");
        assert_eq!(
            config["availability"],
            json!([
                {"topic": "gv2mqtt/availability"},
                {"topic": "gv2mqtt/AABB/availability"},
            ])
        );
    }

    #[test]
    fn bridge_and_recovery_entities_are_unchanged() {
        for (identifier, unique_id) in [
            ("gv2mqtt", "gv2mqtt-purge-caches"),
            ("gv2mqtt-AABB", "sensor-AABB-gv2mqtt-status"),
            ("gv2mqtt-AABB", "gv2mqtt-AABB-request-platform-data"),
        ] {
            let (config, base) = entity(identifier, unique_id);
            assert_eq!(with_device_availability(config.clone(), &base), config);
        }
    }
}

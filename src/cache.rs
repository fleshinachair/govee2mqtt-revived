use anyhow::Context;
use arc_swap::ArcSwap;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sqlite_cache::{Cache, CacheConfig, KeyUpdater};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub static CACHE: Lazy<ArcSwap<Cache>> = Lazy::new(|| {
    open_cache()
        // Only reachable if SQLite cannot even create an in-memory database
        .expect("failed to create an in-memory cache")
        .into()
});

/// Fallback tier for entries the on-disk cache refused to accept.
///
/// If the filesystem holding the cache becomes unwritable (a failing SD card
/// remounting read-only, or a full disk) every write fails. With SQLite as the
/// only tier the value would be recomputed on every request. For `account-info`
/// that means a fresh Govee login each time, which can get the account locked
/// out for 24 hours.
///
/// An entry exists here only while it is newer than what is on disk: a
/// successful write removes it.
static MEMORY_FALLBACK: Lazy<Mutex<HashMap<(String, String), MemoryEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

struct MemoryEntry {
    data: Vec<u8>,
    expires: DateTime<Utc>,
}

fn memory_get(topic: &str, key: &str) -> Option<Vec<u8>> {
    let mut map = MEMORY_FALLBACK.lock().ok()?;
    let map_key = (topic.to_string(), key.to_string());
    match map.get(&map_key) {
        Some(entry) if Utc::now() < entry.expires => Some(entry.data.clone()),
        Some(_) => {
            map.remove(&map_key);
            None
        }
        None => None,
    }
}

fn memory_put(topic: &str, key: &str, data: Vec<u8>, hard_ttl: Duration) {
    if let Ok(mut map) = MEMORY_FALLBACK.lock() {
        map.insert(
            (topic.to_string(), key.to_string()),
            MemoryEntry {
                data,
                expires: Utc::now() + hard_ttl,
            },
        );
    }
}

fn memory_forget(topic: &str, key: &str) {
    if let Ok(mut map) = MEMORY_FALLBACK.lock() {
        map.remove(&(topic.to_string(), key.to_string()));
    }
}

fn memory_clear() {
    if let Ok(mut map) = MEMORY_FALLBACK.lock() {
        map.clear();
    }
}

/// Persist a cache entry. A failed write must not discard a successful result,
/// so it is logged and the value is kept in memory instead.
fn persist(updater: KeyUpdater, data: &[u8], options: &CacheGetOptions<'_>) {
    match updater.write(data, options.hard_ttl) {
        Ok(()) => memory_forget(options.topic, options.key),
        Err(err) => {
            log::warn!(
                "cache write failed for {}/{}: {err:#}. Keeping the value in memory; \
                 the on-disk cache is not persisting (read-only filesystem, or out \
                 of space?)",
                options.topic,
                options.key
            );
            memory_put(options.topic, options.key, data.to_vec(), options.hard_ttl);
        }
    }
}

fn cache_file_name() -> anyhow::Result<PathBuf> {
    let cache_dir = std::env::var("GOVEE_CACHE_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(dirs_next::cache_dir)
        .context("failed to resolve a cache directory; set GOVEE_CACHE_DIR")?;

    Ok(cache_dir.join("govee2mqtt-cache.sqlite"))
}

fn new_cache(conn: sqlite_cache::rusqlite::Connection) -> anyhow::Result<Arc<Cache>> {
    Ok(Arc::new(Cache::new(
        // We have low cardinality and can be pretty relaxed
        CacheConfig {
            flush_gc_ratio: 1024,
            flush_interval: Duration::from_secs(900),
            max_ttl: None,
        },
        conn,
    )?))
}

fn open_cache_file(cache_file: &Path) -> anyhow::Result<Arc<Cache>> {
    if let Some(dir) = cache_file.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {dir:?}"))?;
    }
    let conn = sqlite_cache::rusqlite::Connection::open(cache_file)
        .with_context(|| format!("opening {cache_file:?}"))?;
    new_cache(conn).with_context(|| format!("initializing {cache_file:?}"))
}

/// Open the cache file, or fall back to memory if that is not possible.
/// Running without a persistent cache costs a fresh Govee login and discovery
/// on each restart, which is far better than not running at all.
fn open_cache_or_memory(cache_file: anyhow::Result<PathBuf>) -> anyhow::Result<Arc<Cache>> {
    match cache_file.and_then(|file| open_cache_file(&file)) {
        Ok(cache) => Ok(cache),
        Err(err) => {
            log::error!(
                "Cannot use the on-disk cache: {err:#}. Continuing with an in-memory \
                 cache, which is lost on restart. Check that the directory is writable \
                 by the user running the bridge."
            );
            new_cache(sqlite_cache::rusqlite::Connection::open_in_memory()?)
        }
    }
}

fn open_cache() -> anyhow::Result<Arc<Cache>> {
    // Production-path tests seed synthetic scene metadata through the same
    // global cache API. Never open or modify a developer's persistent cache.
    if cfg!(test) {
        new_cache(sqlite_cache::rusqlite::Connection::open_in_memory()?)
    } else {
        open_cache_or_memory(cache_file_name())
    }
}

fn remove_cache_file() -> anyhow::Result<()> {
    if cfg!(test) {
        return Ok(());
    }
    let cache_file = cache_file_name()?;
    match std::fs::remove_file(&cache_file) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => {
            Err(err).with_context(|| format!("removing cache file {cache_file:?}"))
        }
        _ => Ok(()),
    }
}

/// A cache entry carried across a purge.
struct KeptEntry {
    topic: String,
    key: String,
    data: Vec<u8>,
    ttl: Duration,
}

/// The parts of a stored [`CacheEntry`] that do not depend on its value type.
#[derive(Deserialize)]
struct EntryHeader {
    expires: DateTime<Utc>,
    result: serde_json::Value,
}

/// How much longer a stored entry is valid, if it is a successful result that
/// has not expired. Cached failures and expired entries are not worth keeping.
fn remaining_ttl_of_success(data: &[u8], now: DateTime<Utc>) -> Option<Duration> {
    let header: EntryHeader = serde_json::from_slice(data).ok()?;
    header.result.get("Ok")?;
    (header.expires - now)
        .to_std()
        .ok()
        .filter(|ttl| !ttl.is_zero())
}

fn snapshot_entries(cache: &Cache, keep: &[(&str, &str)]) -> Vec<KeptEntry> {
    let now = Utc::now();
    keep.iter()
        .filter_map(|(topic, key)| {
            let data = memory_get(topic, key)
                .or_else(|| cache.topic(topic).ok()?.get(key).ok()?.map(|v| v.data))?;
            let ttl = remaining_ttl_of_success(&data, now)?;
            Some(KeptEntry {
                topic: topic.to_string(),
                key: key.to_string(),
                data,
                ttl,
            })
        })
        .collect()
}

fn restore_entries(cache: &Cache, kept: Vec<KeptEntry>) {
    for entry in kept {
        let written = cache
            .topic(&entry.topic)
            .and_then(|topic| topic.set(&entry.key, &entry.data, entry.ttl));
        if let Err(err) = written {
            log::warn!(
                "could not carry {}/{} across the cache purge: {err:#}. Keeping it in memory",
                entry.topic,
                entry.key
            );
            memory_put(&entry.topic, &entry.key, entry.data, entry.ttl);
        }
    }
}

/// Discard everything in the cache except the `(topic, key)` entries in `keep`.
///
/// The purge exists to re-fetch device, scene and Tap-to-Run data. The account
/// login is not that kind of data: on an account with two-step verification a
/// new login needs a code from the user's inbox, so dropping it turns the next
/// restart into a failed startup until a fresh code is configured.
pub fn purge_cache(keep: &[(&str, &str)]) -> anyhow::Result<()> {
    let kept = snapshot_entries(&CACHE.load(), keep);
    memory_clear();
    let removed = remove_cache_file();
    CACHE.store(open_cache()?);
    restore_entries(&CACHE.load(), kept);
    removed
}

#[derive(Deserialize, Serialize, Debug)]
struct CacheEntry<T> {
    expires: DateTime<Utc>,
    result: CacheResult<T>,
}

#[derive(Deserialize, Serialize, Debug)]
enum CacheResult<T> {
    Ok(T),
    Err(String),
}

const CACHED_ERROR_MESSAGE: &str =
    "request failure is temporarily cached; retry after the negative-cache window";

impl<T> CacheResult<T> {
    fn into_result(self) -> anyhow::Result<T> {
        match self {
            Self::Ok(v) => Ok(v),
            // Older cache entries may contain the original error text. Never
            // replay that serialized value: it predates the cache-safe error
            // boundary and may contain credentials or response tokens.
            Self::Err(_) => anyhow::bail!(CACHED_ERROR_MESSAGE),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct CacheGetOptions<'a> {
    pub key: &'a str,
    pub topic: &'a str,
    pub soft_ttl: Duration,
    pub hard_ttl: Duration,
    pub negative_ttl: Duration,
    pub allow_stale: bool,
}

pub enum CacheComputeResult<T> {
    Value(T),
    WithTtl(T, Duration),
}

impl<T> CacheComputeResult<T> {
    #[allow(dead_code)]
    pub fn into_inner(self) -> T {
        match self {
            Self::Value(v) | Self::WithTtl(v, _) => v,
        }
    }
}

pub fn invalidate_key(topic: &str, key: &str) -> anyhow::Result<()> {
    memory_forget(topic, key);
    let topic = CACHE.load().topic(topic)?;
    Ok(topic.delete(key)?)
}

/// Marker error: when a cached future returns `Err(anyhow::Error::from(NoCacheError(_)))`,
/// `cache_get` skips the negative-cache write entirely. Use for transient errors where
/// the caller must be able to retry sooner than `negative_ttl` (e.g. 2FA verification
/// codes that expire in ~15 minutes — caching the failure for 15 min would trap the user
/// in a loop matching the code's own validity window).
#[derive(Debug)]
pub struct NoCacheError(pub anyhow::Error);

impl std::fmt::Display for NoCacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for NoCacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

/// Cache an item with a soft TTL; we'll retry the operation
/// if the TTL has expired, but allow stale reads.
///
/// Thin wrapper around [`cache_get_inner`] that uses the global [`CACHE`].
/// Tests use the inner directly with a fresh in-memory cache to stay hermetic.
pub async fn cache_get<T, Fut>(options: CacheGetOptions<'_>, future: Fut) -> anyhow::Result<T>
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + Clone,
    Fut: Future<Output = anyhow::Result<CacheComputeResult<T>>>,
{
    cache_get_inner(&CACHE.load(), options, future).await
}

/// Inner cache_get that takes an explicit `&Cache`. Exposed at crate visibility
/// so unit tests can construct hermetic caches and verify the negative-cache
/// bypass without touching the global singleton.
pub(crate) async fn cache_get_inner<T, Fut>(
    cache: &Cache,
    options: CacheGetOptions<'_>,
    future: Fut,
) -> anyhow::Result<T>
where
    T: Serialize + DeserializeOwned + std::fmt::Debug + Clone,
    Fut: Future<Output = anyhow::Result<CacheComputeResult<T>>>,
{
    let topic = cache.topic(options.topic)?;
    let (updater, current_value) = topic.get_for_update(options.key).await?;
    let now = Utc::now();

    let mut cache_entry: Option<CacheEntry<T>> = None;

    // Memory first: an entry there is newer than the row on disk, which the
    // library keeps returning after it has expired.
    let cached_bytes = memory_get(options.topic, options.key)
        .or_else(|| current_value.map(|current| current.data));

    if let Some(bytes) = &cached_bytes {
        match serde_json::from_slice::<CacheEntry<T>>(bytes) {
            Ok(entry) => {
                let is_legacy_error = matches!(
                    &entry.result,
                    CacheResult::Err(message) if message != CACHED_ERROR_MESSAGE
                );
                if is_legacy_error {
                    // A pre-fix error may contain credentials or response
                    // tokens and may also have a longer negative TTL. Remove
                    // it before recomputing so neither the bytes nor the old
                    // suppression window survive this cache access.
                    topic.delete(options.key)?;
                } else if now < entry.expires {
                    log::trace!("cache hit for {}", options.key);
                    return entry.result.into_result();
                } else {
                    cache_entry.replace(entry);
                }
            }
            Err(err) => {
                log::warn!(
                    "{}",
                    cache_parse_failure_message::<T>(&options, &err, bytes.len())
                );
                // The payload is unusable and may predate the cache-safe
                // error format. Delete it before recomputing so a transient
                // NoCacheError cannot leave secret-bearing bytes behind.
                topic.delete(options.key)?;
            }
        }
    }

    log::trace!("cache miss for {}", options.key);
    let value: anyhow::Result<CacheComputeResult<T>> = future.await;
    match value {
        Ok(CacheComputeResult::WithTtl(value, ttl)) => {
            let entry = CacheEntry {
                expires: Utc::now() + ttl,
                result: CacheResult::Ok(value.clone()),
            };

            let data = serde_json::to_string_pretty(&entry)?;
            persist(updater, data.as_bytes(), &options);
            Ok(value)
        }
        Ok(CacheComputeResult::Value(value)) => {
            let entry = CacheEntry {
                expires: Utc::now() + options.soft_ttl,
                result: CacheResult::Ok(value.clone()),
            };

            let data = serde_json::to_string_pretty(&entry)?;
            persist(updater, data.as_bytes(), &options);
            Ok(value)
        }
        Err(err) => {
            // Opt-out: callers can return Err(NoCacheError(...).into()) to bypass
            // both the stale-read path and the negative-cache write. The next call
            // will re-execute the future instead of returning the cached failure.
            if err.downcast_ref::<NoCacheError>().is_some() {
                log::trace!(
                    "cache_get: NoCacheError marker, skipping negative cache write for {}",
                    options.key
                );
                return Err(err);
            }
            match cache_entry.take() {
                Some(mut entry)
                    if options.allow_stale && matches!(&entry.result, CacheResult::Ok(_)) =>
                {
                    entry.expires = Utc::now() + options.negative_ttl;

                    log::warn!("{err:#}, will use prior results");

                    let data = serde_json::to_string_pretty(&entry)?;
                    persist(updater, data.as_bytes(), &options);

                    entry.result.into_result()
                }
                _ => {
                    let entry: CacheEntry<T> = CacheEntry {
                        expires: Utc::now() + options.negative_ttl,
                        // Arbitrary errors are not safe cache data. Return the
                        // detailed error to the immediate caller, but persist
                        // only an opaque marker for subsequent cache hits.
                        result: CacheResult::Err(CACHED_ERROR_MESSAGE.to_string()),
                    };

                    let data = serde_json::to_string_pretty(&entry)?;
                    persist(updater, data.as_bytes(), &options);
                    Err(err)
                }
            }
        }
    }
}

fn cache_parse_failure_message<T>(
    options: &CacheGetOptions<'_>,
    error: &serde_json::Error,
    body_len: usize,
) -> String {
    format!(
        "Error parsing cached {} for topic {:?}, key {:?}: JSON {:?} error at line {}, column {}. \
         Cached payload withheld ({} bytes)",
        std::any::type_name::<CacheEntry<T>>(),
        options.topic,
        options.key,
        error.classify(),
        error.line(),
        error.column(),
        body_len
    )
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Fresh in-memory SQLite-backed cache for each test. No global state, no
    /// cross-test pollution, no risk of writing to the user's real cache file.
    fn fresh_cache() -> Cache {
        let conn =
            sqlite_cache::rusqlite::Connection::open_in_memory().expect("open in-memory sqlite");
        Cache::new(
            CacheConfig {
                flush_gc_ratio: 1024,
                flush_interval: Duration::from_secs(900),
                max_ttl: None,
            },
            conn,
        )
        .expect("Cache::new")
    }

    fn opts(key: &'static str, negative_ttl: Duration) -> CacheGetOptions<'static> {
        CacheGetOptions {
            topic: "test",
            key,
            soft_ttl: Duration::from_secs(60),
            hard_ttl: Duration::from_secs(60),
            negative_ttl,
            allow_stale: false,
        }
    }

    #[tokio::test]
    async fn purge_carries_a_live_success_into_the_new_cache() {
        let old = fresh_cache();
        let kept_options = CacheGetOptions {
            topic: "undoc-api",
            ..opts("purge-kept-login", Duration::from_secs(10))
        };
        cache_get_inner(&old, kept_options, async {
            Ok::<_, anyhow::Error>(CacheComputeResult::Value("TOKEN".to_string()))
        })
        .await
        .expect("seed the login entry");
        cache_get_inner(&old, opts("purge-dropped-scenes", Duration::ZERO), async {
            Ok::<_, anyhow::Error>(CacheComputeResult::Value("SCENES".to_string()))
        })
        .await
        .expect("seed the device data entry");

        let kept = snapshot_entries(&old, &[("undoc-api", "purge-kept-login")]);
        let new = fresh_cache();
        restore_entries(&new, kept);

        let calls = AtomicUsize::new(0);
        let login: String = cache_get_inner(&new, kept_options, async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, anyhow::Error>(CacheComputeResult::Value("NEW LOGIN".to_string()))
        })
        .await
        .expect("read the login entry after the purge");

        assert_eq!(login, "TOKEN");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "a kept login must not trigger a new login"
        );
        assert!(
            new.topic("test")
                .expect("open test topic")
                .get("purge-dropped-scenes")
                .expect("read dropped entry")
                .is_none(),
            "entries outside the keep list must not survive the purge"
        );
    }

    #[test]
    fn purge_does_not_keep_failures_or_expired_entries() {
        let now = Utc::now();
        let live = br#"{"expires": "2999-01-01T00:00:00Z", "result": {"Ok": "TOKEN"}}"#;
        let failure = br#"{"expires": "2999-01-01T00:00:00Z", "result": {"Err": "x"}}"#;
        let expired = br#"{"expires": "2000-01-01T00:00:00Z", "result": {"Ok": "TOKEN"}}"#;

        assert!(remaining_ttl_of_success(live, now).is_some());
        assert!(remaining_ttl_of_success(failure, now).is_none());
        assert!(remaining_ttl_of_success(expired, now).is_none());
        assert!(remaining_ttl_of_success(b"not json", now).is_none());
    }

    #[test]
    fn corrupt_cache_diagnostic_withholds_serialized_value() {
        let body = br#"{
            "expires": "2999-01-01T00:00:00Z",
            "result": {"Ok": "TOKEN_SENTINEL"}
        }"#;
        let error = serde_json::from_slice::<CacheEntry<u64>>(body)
            .expect_err("string cache value must not deserialize as u64");
        let options = opts("account-info", Duration::from_secs(10));
        let rendered = cache_parse_failure_message::<u64>(&options, &error, body.len());

        assert!(!rendered.contains("TOKEN_SENTINEL"), "{rendered}");
        assert!(rendered.contains("topic \"test\""), "{rendered}");
        assert!(rendered.contains("key \"account-info\""), "{rendered}");
        assert!(rendered.contains("JSON Data error"), "{rendered}");
        assert!(rendered.contains("line "), "{rendered}");
        assert!(rendered.contains("column "), "{rendered}");
        assert!(
            rendered.contains(&format!("Cached payload withheld ({} bytes)", body.len())),
            "{rendered}"
        );
    }

    #[tokio::test]
    async fn caches_ok_value_so_future_runs_only_once() {
        let cache = fresh_cache();
        let calls = AtomicUsize::new(0);

        let f1 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, anyhow::Error>(CacheComputeResult::Value(42_u64))
        };
        let v1 = cache_get_inner(&cache, opts("ok", Duration::from_secs(10)), f1)
            .await
            .unwrap();

        let f2 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, anyhow::Error>(CacheComputeResult::Value(42_u64))
        };
        let v2 = cache_get_inner(&cache, opts("ok", Duration::from_secs(10)), f2)
            .await
            .unwrap();

        assert_eq!(v1, 42);
        assert_eq!(v2, 42);
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "second call should hit cache, not re-execute"
        );
    }

    #[tokio::test]
    async fn plain_err_writes_negative_cache_so_future_runs_only_once() {
        let cache = fresh_cache();
        let calls = AtomicUsize::new(0);

        let f1 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err::<CacheComputeResult<u64>, _>(anyhow::anyhow!("boom"))
        };
        let r1 = cache_get_inner(&cache, opts("err", Duration::from_secs(60)), f1).await;

        let f2 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err::<CacheComputeResult<u64>, _>(anyhow::anyhow!("boom"))
        };
        let r2 = cache_get_inner(&cache, opts("err", Duration::from_secs(60)), f2).await;

        assert!(r1.is_err());
        assert!(r2.is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "plain Err must be negative-cached; future must NOT re-execute"
        );
    }

    #[tokio::test]
    async fn negative_cache_never_persists_or_replays_error_details() {
        const SECRET: &str = "OPTED_IN_RESPONSE_SECRET_SENTINEL";

        let cache = fresh_cache();
        let calls = AtomicUsize::new(0);

        let first = cache_get_inner(
            &cache,
            opts("secret-error", Duration::from_secs(60)),
            async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<CacheComputeResult<u64>, _>(anyhow::anyhow!(SECRET))
            },
        )
        .await
        .expect_err("the immediate request must return its detailed failure");
        assert!(format!("{first:#}").contains(SECRET), "{first:#}");

        let stored = cache
            .topic("test")
            .expect("open test topic")
            .get("secret-error")
            .expect("read cached error")
            .expect("negative-cache entry must exist");
        let stored = String::from_utf8(stored.data).expect("cache entry must be UTF-8 JSON");
        assert!(
            !stored.contains(SECRET),
            "secret persisted in cache: {stored}"
        );
        assert!(stored.contains(CACHED_ERROR_MESSAGE), "{stored}");

        let second = cache_get_inner(
            &cache,
            opts("secret-error", Duration::from_secs(60)),
            async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<CacheComputeResult<u64>, _>(anyhow::anyhow!(
                    "future must not execute during the negative-cache window"
                ))
            },
        )
        .await
        .expect_err("the cached failure must remain an error");
        let rendered = format!("{second:#}");

        assert!(
            !rendered.contains(SECRET),
            "cached error replayed secret: {rendered}"
        );
        assert!(rendered.contains(CACHED_ERROR_MESSAGE), "{rendered}");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "negative-cache hit must not execute the second future"
        );
    }

    #[tokio::test]
    async fn failed_refresh_reuses_only_a_stale_success() {
        let cache = fresh_cache();
        let options = CacheGetOptions {
            topic: "test",
            key: "stale-success",
            soft_ttl: Duration::ZERO,
            hard_ttl: Duration::from_secs(60),
            negative_ttl: Duration::from_secs(60),
            allow_stale: true,
        };

        let first = cache_get_inner(&cache, options, async {
            Ok::<_, anyhow::Error>(CacheComputeResult::Value(42_u64))
        })
        .await
        .expect("seed stale successful value");
        let second = cache_get_inner(&cache, options, async {
            Err::<CacheComputeResult<u64>, _>(anyhow::anyhow!("refresh failed"))
        })
        .await
        .expect("failed refresh must return the stale success");

        assert_eq!(first, 42);
        assert_eq!(second, 42);
    }

    #[tokio::test]
    async fn expired_cached_error_returns_the_current_failure() {
        let cache = fresh_cache();
        let options = CacheGetOptions {
            topic: "test",
            key: "expired-error",
            soft_ttl: Duration::ZERO,
            hard_ttl: Duration::from_secs(60),
            negative_ttl: Duration::ZERO,
            allow_stale: true,
        };

        cache_get_inner(&cache, options, async {
            Err::<CacheComputeResult<u64>, _>(anyhow::anyhow!("first failure"))
        })
        .await
        .expect_err("seed expired negative-cache entry");

        let second = cache_get_inner(&cache, options, async {
            Err::<CacheComputeResult<u64>, _>(anyhow::anyhow!("CURRENT_FAILURE_SENTINEL"))
        })
        .await
        .expect_err("expired cached error must execute and return the current failure");
        let rendered = format!("{second:#}");

        assert!(rendered.contains("CURRENT_FAILURE_SENTINEL"), "{rendered}");
        assert!(!rendered.contains("first failure"), "{rendered}");
    }

    #[tokio::test]
    async fn fresh_legacy_cached_error_is_removed_before_recomputation() {
        const SECRET: &str = "LEGACY_SECRET_SENTINEL";

        let cache = fresh_cache();
        let topic = cache.topic("test").expect("open test topic");
        let (updater, _) = topic
            .get_for_update("legacy-error")
            .await
            .expect("open legacy error for update");
        let legacy = CacheEntry::<u64> {
            expires: Utc::now() + Duration::from_secs(60),
            result: CacheResult::Err(SECRET.to_string()),
        };
        let legacy = serde_json::to_vec(&legacy).expect("serialize legacy cached error");
        updater
            .write(&legacy, Duration::from_secs(60))
            .expect("seed legacy cached error");

        let calls = AtomicUsize::new(0);
        let error = cache_get_inner(
            &cache,
            opts("legacy-error", Duration::from_secs(60)),
            async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<CacheComputeResult<u64>, _>(
                    NoCacheError(anyhow::anyhow!("CURRENT_TRANSIENT_FAILURE")).into(),
                )
            },
        )
        .await
        .expect_err("the recomputed transient failure must be returned");
        let rendered = format!("{error:#}");

        assert!(!rendered.contains(SECRET), "{rendered}");
        assert!(rendered.contains("CURRENT_TRANSIENT_FAILURE"), "{rendered}");
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        assert!(
            topic
                .get("legacy-error")
                .expect("read removed legacy cached error")
                .is_none(),
            "legacy error must be deleted when recomputation is not cacheable"
        );
    }

    #[tokio::test]
    async fn expired_legacy_cached_error_is_removed_before_no_cache_error() {
        const SECRET: &str = "EXPIRED_LEGACY_SECRET_SENTINEL";

        let cache = fresh_cache();
        let calls = AtomicUsize::new(0);
        let topic = cache.topic("test").expect("open test topic");
        let (updater, _) = topic
            .get_for_update("expired-legacy-error")
            .await
            .expect("open expired legacy error for update");
        let legacy = CacheEntry::<u64> {
            expires: Utc::now() - Duration::from_secs(1),
            result: CacheResult::Err(SECRET.to_string()),
        };
        let legacy = serde_json::to_vec(&legacy).expect("serialize expired legacy cached error");
        updater
            .write(&legacy, Duration::from_secs(60))
            .expect("seed expired legacy cached error");

        let error = cache_get_inner(
            &cache,
            opts("expired-legacy-error", Duration::from_secs(60)),
            async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<CacheComputeResult<u64>, _>(
                    NoCacheError(anyhow::anyhow!("CURRENT_TRANSIENT_FAILURE")).into(),
                )
            },
        )
        .await
        .expect_err("the current transient failure must be returned");
        let rendered = format!("{error:#}");

        assert!(error.downcast_ref::<NoCacheError>().is_some(), "{rendered}");
        assert!(!rendered.contains(SECRET), "{rendered}");
        assert!(rendered.contains("CURRENT_TRANSIENT_FAILURE"), "{rendered}");
        assert!(
            topic
                .get("expired-legacy-error")
                .expect("read removed expired legacy error")
                .is_none(),
            "expired legacy error must be deleted before returning NoCacheError"
        );

        cache_get_inner(
            &cache,
            opts("expired-legacy-error", Duration::from_secs(60)),
            async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err::<CacheComputeResult<u64>, _>(
                    NoCacheError(anyhow::anyhow!("SECOND_TRANSIENT_FAILURE")).into(),
                )
            },
        )
        .await
        .expect_err("the second transient failure must also be recomputed");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "removing a legacy error must preserve the no-cache retry contract"
        );
    }

    #[tokio::test]
    async fn malformed_cached_value_is_removed_before_no_cache_error() {
        const SECRET: &str = "MALFORMED_CACHE_SECRET_SENTINEL";

        let cache = fresh_cache();
        let topic = cache.topic("test").expect("open test topic");
        let (updater, _) = topic
            .get_for_update("malformed-error")
            .await
            .expect("open malformed error for update");
        updater
            .write(
                format!(r#"{{"result":{{"Err":"{SECRET}"}}}}"#).as_bytes(),
                Duration::from_secs(60),
            )
            .expect("seed malformed cached error");

        cache_get_inner(
            &cache,
            opts("malformed-error", Duration::from_secs(60)),
            async {
                Err::<CacheComputeResult<u64>, _>(
                    NoCacheError(anyhow::anyhow!("CURRENT_TRANSIENT_FAILURE")).into(),
                )
            },
        )
        .await
        .expect_err("the current transient failure must be returned");

        assert!(
            topic
                .get("malformed-error")
                .expect("read removed malformed cached value")
                .is_none(),
            "malformed cache data must be deleted before returning NoCacheError"
        );
    }

    /// The load-bearing test for the bug-fix: a future returning a NoCacheError
    /// MUST cause cache_get to skip the negative-cache write entirely. The next
    /// call must re-execute the future, not return a cached failure. This is
    /// what lets a 2FA verification code retry succeed inside its 15-min
    /// validity window.
    #[tokio::test]
    async fn no_cache_error_skips_negative_cache_so_future_re_executes() {
        let cache = fresh_cache();
        let calls = AtomicUsize::new(0);

        let f1 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err::<CacheComputeResult<u64>, _>(NoCacheError(anyhow::anyhow!("transient")).into())
        };
        let r1 = cache_get_inner(&cache, opts("nocache", Duration::from_secs(60)), f1).await;

        let f2 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err::<CacheComputeResult<u64>, _>(NoCacheError(anyhow::anyhow!("transient")).into())
        };
        let r2 = cache_get_inner(&cache, opts("nocache", Duration::from_secs(60)), f2).await;

        assert!(r1.is_err());
        assert!(r2.is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "NoCacheError must NOT be cached; second call MUST re-execute the future"
        );
    }

    #[tokio::test]
    async fn no_cache_error_message_propagates_and_downcast_survives() {
        let cache = fresh_cache();
        let f = async {
            Err::<CacheComputeResult<u64>, _>(
                NoCacheError(anyhow::anyhow!("specific message")).into(),
            )
        };
        let err = cache_get_inner(&cache, opts("msg", Duration::from_secs(60)), f)
            .await
            .unwrap_err();

        assert!(
            format!("{err:#}").contains("specific message"),
            "error message lost: got {err:#}"
        );
        assert!(
            err.downcast_ref::<NoCacheError>().is_some(),
            "downcast must survive cache_get's pass-through"
        );
    }

    /// Regression guard: existing successful cache entries must still be
    /// served on subsequent calls regardless of whether subsequent futures
    /// would have returned NoCacheError. Just confirms cache_get's read path
    /// is independent of the future type.
    #[tokio::test]
    async fn fresh_ok_entry_is_returned_without_running_future() {
        let cache = fresh_cache();
        let calls = AtomicUsize::new(0);

        let f1 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok::<_, anyhow::Error>(CacheComputeResult::WithTtl(
                "hello".to_string(),
                Duration::from_secs(30),
            ))
        };
        let _ = cache_get_inner(&cache, opts("ttl-ok", Duration::from_secs(60)), f1)
            .await
            .unwrap();

        // Second call: future would NoCacheError, but cache should not run it.
        let f2 = async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err::<CacheComputeResult<String>, _>(
                NoCacheError(anyhow::anyhow!("would bypass")).into(),
            )
        };
        let v2 = cache_get_inner(&cache, opts("ttl-ok", Duration::from_secs(60)), f2)
            .await
            .unwrap();
        assert_eq!(v2, "hello");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn memory_fallback_honours_ttl_topic_and_key() {
        let ttl = Duration::from_secs(60);
        memory_put("mem-a", "shared", b"a".to_vec(), ttl);
        memory_put("mem-b", "shared", b"b".to_vec(), ttl);
        memory_put("mem-a", "expired", b"x".to_vec(), Duration::ZERO);

        assert_eq!(memory_get("mem-a", "shared").as_deref(), Some(&b"a"[..]));
        assert_eq!(memory_get("mem-b", "shared").as_deref(), Some(&b"b"[..]));
        assert_eq!(memory_get("mem-a", "expired"), None);

        memory_forget("mem-a", "shared");
        assert_eq!(memory_get("mem-a", "shared"), None);
        assert_eq!(memory_get("mem-b", "shared").as_deref(), Some(&b"b"[..]));
        memory_forget("mem-b", "shared");
    }

    /// wez/govee2mqtt#702: when the disk cache stops accepting writes while it
    /// still holds an expired entry, the new result must be kept and reused.
    #[tokio::test]
    async fn failed_write_keeps_result_despite_expired_disk_entry() {
        let path =
            std::env::temp_dir().join(format!("govee-cache-test-{}.sqlite", uuid::Uuid::new_v4()));
        let open = || sqlite_cache::rusqlite::Connection::open(&path).expect("open sqlite");
        let cache = Cache::new(
            CacheConfig {
                flush_gc_ratio: 1024,
                flush_interval: Duration::from_secs(900),
                max_ttl: None,
            },
            open(),
        )
        .expect("Cache::new");
        let options = CacheGetOptions {
            topic: "test-unwritable",
            ..opts("account", Duration::from_secs(10))
        };

        // An entry that is already expired but still on disk
        cache_get_inner(&cache, options, async {
            Ok(CacheComputeResult::WithTtl(1_u64, Duration::ZERO))
        })
        .await
        .unwrap();

        // From now on every write fails
        let other = open();
        let table: String = other
            .query_row(
                "select name from sqlite_master where type = 'table' and name like 'topic_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        other
            .execute_batch(&format!(
                "create trigger fail_writes before insert on {table} \
                 begin select raise(abort, 'simulated unwritable disk'); end;"
            ))
            .unwrap();

        let calls = AtomicUsize::new(0);
        for _ in 0..3 {
            let value = cache_get_inner(&cache, options, async {
                calls.fetch_add(1, Ordering::SeqCst);
                Ok(CacheComputeResult::Value(2_u64))
            })
            .await
            .unwrap();
            assert_eq!(value, 2);
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1, "must not recompute");

        memory_forget("test-unwritable", "account");
        drop(other);
        drop(cache);
        let _ = std::fs::remove_file(&path);
    }

    /// wez/govee2mqtt#536: a cache file that cannot be opened must not panic.
    #[tokio::test]
    async fn unusable_cache_location_falls_back_to_memory() {
        // A regular file where the cache directory should be
        let blocker =
            std::env::temp_dir().join(format!("govee-cache-blocker-{}", uuid::Uuid::new_v4()));
        std::fs::write(&blocker, b"this is not a database or a directory").unwrap();

        for cache_file in [
            Ok(blocker.join("cache.sqlite")),
            // The path exists but is not a database
            Ok(blocker.clone()),
            Err(anyhow::anyhow!("no cache directory")),
        ] {
            let cache = open_cache_or_memory(cache_file).expect("in-memory fallback");
            let value = cache_get_inner(&cache, opts("fallback", Duration::from_secs(10)), async {
                Ok(CacheComputeResult::Value(3_u64))
            })
            .await
            .unwrap();
            assert_eq!(value, 3);
        }

        let _ = std::fs::remove_file(&blocker);
    }

    #[test]
    fn cache_directory_is_created_when_missing() {
        let dir = std::env::temp_dir().join(format!("govee-cache-dir-{}", uuid::Uuid::new_v4()));
        let file = dir.join("nested").join("cache.sqlite");
        let cache = open_cache_file(&file).expect("open");
        assert!(file.is_file());
        drop(cache);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

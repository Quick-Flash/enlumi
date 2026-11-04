//! Small shared, demand-driven tile service for the standalone demo.
//! No prefetch: two blocking workers claim only current visible demand.
use super::camera::TileId;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    hash::Hasher,
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use zgui::prelude::*;

const MEMORY: usize = 384 * 1024 * 1024;
const DISK: u64 = 2 * 1024 * 1024 * 1024;
const MAX_BODY: u64 = 4 * 1024 * 1024;
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    #[default]
    OpenStreetMap,
    GoogleRoad,
    GoogleSatellite,
    GoogleHybrid,
    BingRoad,
    BingSatellite,
    BingHybrid,
    EsriStreet,
    EsriImagery,
    EsriTerrain,
    MapboxStreets,
    MapboxSatellite,
}
impl Provider {
    pub const ALL: [Self; 12] = [
        Self::OpenStreetMap,
        Self::GoogleRoad,
        Self::GoogleSatellite,
        Self::GoogleHybrid,
        Self::BingRoad,
        Self::BingSatellite,
        Self::BingHybrid,
        Self::EsriStreet,
        Self::EsriImagery,
        Self::EsriTerrain,
        Self::MapboxStreets,
        Self::MapboxSatellite,
    ];
    pub fn key(self) -> &'static str {
        match self {
            Self::OpenStreetMap => "osm",
            Self::GoogleRoad => "google-road",
            Self::GoogleSatellite => "google-satellite",
            Self::GoogleHybrid => "google-hybrid",
            Self::BingRoad => "bing-road",
            Self::BingSatellite => "bing-satellite",
            Self::BingHybrid => "bing-hybrid",
            Self::EsriStreet => "esri-street",
            Self::EsriImagery => "esri-imagery",
            Self::EsriTerrain => "esri-terrain",
            Self::MapboxStreets => "mapbox-streets",
            Self::MapboxSatellite => "mapbox-satellite",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::OpenStreetMap => "OpenStreetMap",
            Self::GoogleRoad => "Google Road",
            Self::GoogleSatellite => "Google Satellite",
            Self::GoogleHybrid => "Google Hybrid",
            Self::BingRoad => "Bing Road",
            Self::BingSatellite => "Bing Satellite",
            Self::BingHybrid => "Bing Hybrid",
            Self::EsriStreet => "Esri World Street",
            Self::EsriImagery => "Esri World Imagery",
            Self::EsriTerrain => "Esri Terrain",
            Self::MapboxStreets => "Mapbox Streets",
            Self::MapboxSatellite => "Mapbox Satellite",
        }
    }
    pub fn parse(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.key() == key)
    }
    pub fn expand_tiles(self, tiles: Vec<TileId>) -> Vec<TileId> {
        let minimum_zoom = match self {
            Self::BingRoad | Self::BingSatellite | Self::BingHybrid => 1,
            _ => 0,
        };
        let mut expanded = tiles
            .into_iter()
            .flat_map(|tile| {
                if tile.z >= minimum_zoom {
                    return vec![tile];
                }
                let shift = minimum_zoom - tile.z;
                let side = 1_i64 << shift;
                (0..side)
                    .flat_map(move |y| {
                        (0..side).map(move |x| TileId {
                            z: minimum_zoom,
                            x: tile.x * side + x,
                            y: tile.y * side as u32 + y as u32,
                        })
                    })
                    .collect()
            })
            .collect::<Vec<_>>();
        let mut unique = BTreeSet::new();
        expanded.retain(|tile| unique.insert(*tile));
        expanded
    }
    pub fn attribution(self) -> &'static str {
        match self {
            Self::OpenStreetMap => "© OpenStreetMap contributors",
            Self::GoogleRoad | Self::GoogleSatellite | Self::GoogleHybrid => "© Google",
            Self::BingRoad | Self::BingSatellite | Self::BingHybrid => "© Microsoft Bing",
            Self::EsriStreet | Self::EsriImagery | Self::EsriTerrain => "© Esri",
            Self::MapboxStreets | Self::MapboxSatellite => "© Mapbox",
        }
    }
    pub fn attribution_url(self) -> &'static str {
        match self {
            Self::OpenStreetMap => "https://www.openstreetmap.org/copyright",
            Self::GoogleRoad | Self::GoogleSatellite | Self::GoogleHybrid => {
                "https://maps.google.com/"
            }
            Self::BingRoad | Self::BingSatellite | Self::BingHybrid => {
                "https://www.microsoft.com/en-us/maps/"
            }
            Self::EsriStreet | Self::EsriImagery | Self::EsriTerrain => {
                "https://www.esri.com/en-us/legal/terms/full-master-agreement"
            }
            Self::MapboxStreets | Self::MapboxSatellite => "https://www.mapbox.com/about/maps/",
        }
    }
    fn url(self, key: Key) -> Result<String, String> {
        let (x, y, z) = (key.x, key.y, key.z);
        let server = (x.wrapping_mul(3).wrapping_add(y)) % 4;
        Ok(match self {
            Self::OpenStreetMap => format!("https://tile.openstreetmap.org/{z}/{x}/{y}.png"),
            Self::GoogleRoad | Self::GoogleSatellite | Self::GoogleHybrid => {
                let layer = match self {
                    Self::GoogleRoad => "m",
                    Self::GoogleSatellite => "s",
                    _ => "y",
                };
                format!("https://mt{server}.google.com/vt/lyrs={layer}&x={x}&y={y}&z={z}")
            }
            Self::BingRoad | Self::BingSatellite | Self::BingHybrid => {
                let layer = match self {
                    Self::BingRoad => 'r',
                    Self::BingSatellite => 'a',
                    _ => 'h',
                };
                let mut quad = String::with_capacity(z as usize);
                for bit in (0..z).rev() {
                    let mask = 1 << bit;
                    let digit = u8::from(x & mask != 0) + 2 * u8::from(y & mask != 0);
                    quad.push(char::from(b'0' + digit));
                }
                format!(
                    "https://ecn.t{server}.tiles.virtualearth.net/tiles/{layer}{quad}.jpg?g=2981&mkt=en-US"
                )
            }
            Self::EsriStreet | Self::EsriImagery | Self::EsriTerrain => {
                let layer = match self {
                    Self::EsriStreet => "World_Street_Map",
                    Self::EsriImagery => "World_Imagery",
                    _ => "World_Terrain_Base",
                };
                format!(
                    "https://server.arcgisonline.com/ArcGIS/rest/services/{layer}/MapServer/tile/{z}/{y}/{x}"
                )
            }
            Self::MapboxStreets | Self::MapboxSatellite => {
                let token = std::env::var("ENLUMI_MAPBOX_TOKEN")
                    .map_err(|_| "Set ENLUMI_MAPBOX_TOKEN to use Mapbox".to_string())?;
                let style = if self == Self::MapboxStreets {
                    "streets-v12"
                } else {
                    "satellite-v9"
                };
                format!(
                    "https://api.mapbox.com/styles/v1/mapbox/{style}/tiles/256/{z}/{x}/{y}?access_token={token}"
                )
            }
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Key {
    pub dem: bool,
    pub provider: Provider,
    pub z: u8,
    pub x: u32,
    pub y: u32,
}
impl Key {
    pub fn raster(tile: TileId) -> Self {
        Self {
            dem: false,
            provider: Provider::OpenStreetMap,
            z: tile.z,
            x: tile.canonical_x(),
            y: tile.y,
        }
    }
    pub fn raster_from(tile: TileId, provider: Provider) -> Self {
        Self {
            provider,
            ..Self::raster(tile)
        }
    }
    pub fn elevation(tile: TileId) -> Self {
        let z = tile.z.min(14);
        let shift = tile.z - z;
        Self {
            dem: true,
            provider: Provider::OpenStreetMap,
            z,
            x: tile.canonical_x() >> shift,
            y: tile.y >> shift,
        }
    }
    fn relative(self) -> PathBuf {
        PathBuf::from(if self.dem {
            "mapterhorn"
        } else {
            self.provider.key()
        })
        .join(self.z.to_string())
        .join(self.x.to_string())
        .join(format!(
            "{}.{}",
            self.y,
            if self.dem { "webp" } else { "png" }
        ))
    }
}

#[derive(Clone)]
pub struct Config {
    pub cache: PathBuf,
    pub local: Option<PathBuf>,
    pub network: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            cache: std::env::var_os("ENLUMI_MAP_CACHE")
                .map_or_else(|| PathBuf::from("target/map-preview-cache"), PathBuf::from),
            local: std::env::var_os("ENLUMI_MAP_LOCAL").map(PathBuf::from),
            network: true,
        }
    }
}

pub struct Raster {
    pub version: u64,
    pub encoded: Vec<u8>,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}
pub struct Elevation {
    pub version: u64,
    pub heights: Vec<f32>,
    pub width: u32,
    pub height: u32,
}
pub enum Payload {
    Raster(Raster),
    Elevation(Elevation),
}
impl Payload {
    pub fn version(&self) -> u64 {
        match self {
            Self::Raster(r) => r.version,
            Self::Elevation(e) => e.version,
        }
    }
    fn bytes(&self) -> usize {
        match self {
            Self::Raster(r) => r.rgba.len() + 2 * r.encoded.len(),
            Self::Elevation(e) => e.heights.len() * 4,
        }
    }
}
pub struct Ready {
    pub payload: Arc<Payload>,
    pub image: Option<zgui_image::ImageBytes>,
}
struct Entry {
    value: Result<Ready, String>,
    used: u64,
}
#[derive(Clone, PartialEq, Eq)]
struct Demand {
    keys: Vec<Key>,
    online: bool,
}
#[derive(Default)]
struct Cache {
    entries: BTreeMap<Key, Entry>,
    demands: BTreeMap<u64, Demand>,
    tick: u64,
}
impl Cache {
    fn priorities(&self) -> BTreeMap<Key, usize> {
        let mut priorities = BTreeMap::<Key, usize>::new();
        for demand in self.demands.values() {
            for (rank, &key) in demand.keys.iter().enumerate() {
                priorities
                    .entry(key)
                    .and_modify(|p| *p = (*p).min(rank))
                    .or_insert(rank);
            }
        }
        priorities
    }
}
#[derive(Default)]
struct Work {
    desired: BTreeSet<Key>,
    priorities: BTreeMap<Key, usize>,
    claimed: BTreeSet<Key>,
    known: BTreeSet<Key>,
    online: BTreeSet<Key>,
    closed: bool,
    blocked: [Option<Instant>; 2],
}
impl Work {
    fn next(&self, worker: usize, now: Instant) -> Option<Key> {
        self.desired
            .iter()
            .filter(|key| {
                !self.claimed.contains(key)
                    && !self.known.contains(key)
                    && self.blocked[usize::from(key.dem)].is_none_or(|until| until <= now)
            })
            .min_by_key(|key| {
                (
                    key.dem != (worker == 1),
                    self.priorities.get(key).copied().unwrap_or(usize::MAX),
                )
            })
            .copied()
    }
}
struct Shared {
    loads: AtomicU64,
    work: Mutex<Work>,
    changed: Condvar,
}
struct Ui {
    cache: std::cell::RefCell<Cache>,
    shared: Arc<Shared>,
}
#[derive(Clone, Copy)]
pub struct Service {
    ui: StoredValue<Rc<Ui>, LocalStorage>,
    pub revision: RwSignal<u64, LocalStorage>,
}

impl Service {
    #[cfg(test)]
    pub fn loads(self) -> u64 {
        self.ui
            .with_value(|ui| ui.shared.loads.load(Ordering::Relaxed))
    }
    #[cfg(test)]
    pub fn requested(self) -> Vec<Key> {
        self.ui.with_value(|ui| {
            ui.shared
                .work
                .lock()
                .unwrap()
                .desired
                .iter()
                .copied()
                .collect()
        })
    }
    #[cfg(test)]
    pub fn failed(self) -> bool {
        let ui = self.ui.get_value();
        let cache = ui.cache.borrow();
        ui.shared
            .work
            .lock()
            .unwrap()
            .desired
            .iter()
            .any(|key| cache.entries.get(key).is_some_and(|e| e.value.is_err()))
    }
    #[cfg(test)]
    pub fn complete(self) -> bool {
        let ui = self.ui.get_value();
        let cache = ui.cache.borrow();
        let work = ui.shared.work.lock().unwrap();
        !work.desired.is_empty()
            && work
                .desired
                .iter()
                .all(|key| cache.entries.get(key).is_some_and(|e| e.value.is_ok()))
    }
    pub fn new(config: Config) -> Self {
        let shared = Arc::new(Shared {
            loads: AtomicU64::new(0),
            work: Mutex::new(Work::default()),
            changed: Condvar::new(),
        });
        let ui = Rc::new(Ui {
            cache: Default::default(),
            shared: shared.clone(),
        });
        let revision = RwSignal::new_local(0_u64);
        let (tx, rx) = async_channel::bounded::<(Key, Result<Arc<Payload>, String>)>(8);
        for worker in 0..2 {
            let shared = shared.clone();
            let tx = tx.clone();
            let config = config.clone();
            std::thread::spawn(move || {
                let tls = ureq::tls::TlsConfig::builder();
                #[cfg(any(windows, target_os = "macos"))]
                let tls = tls
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier);
                let agent: ureq::Agent = ureq::Agent::config_builder()
                    .timeout_global(Some(Duration::from_secs(20)))
                    .timeout_connect(Some(Duration::from_secs(10)))
                    .https_only(true)
                    .tls_config(tls.build())
                    .build()
                    .into();
                loop {
                    let (key, online) = {
                        let mut work = shared.work.lock().unwrap();
                        loop {
                            if work.closed {
                                return;
                            }
                            if let Some(key) = work.next(worker, Instant::now()) {
                                work.claimed.insert(key);
                                break (key, work.online.contains(&key));
                            }
                            let delay = work
                                .desired
                                .iter()
                                .filter(|key| {
                                    !work.claimed.contains(key) && !work.known.contains(key)
                                })
                                .filter_map(|key| work.blocked[usize::from(key.dem)])
                                .filter(|until| *until > Instant::now())
                                .min();
                            work = if let Some(until) = delay {
                                shared
                                    .changed
                                    .wait_timeout(
                                        work,
                                        until.saturating_duration_since(Instant::now()),
                                    )
                                    .unwrap()
                                    .0
                            } else {
                                shared.changed.wait(work).unwrap()
                            };
                        }
                    };
                    shared.loads.fetch_add(1, Ordering::Relaxed);
                    match load(&config, &agent, key, online) {
                        Ok((payload, refresh)) => {
                            let payload = Arc::new(payload);
                            if tx.send_blocking((key, Ok(payload.clone()))).is_err() {
                                return;
                            }
                            if refresh {
                                match load_impl(&config, &agent, key, online, false) {
                                    Ok((updated, _)) if updated.version() != payload.version() => {
                                        if tx.send_blocking((key, Ok(Arc::new(updated)))).is_err() {
                                            return;
                                        }
                                    }
                                    Err(message)
                                        if message.contains("429") || message.contains("503") =>
                                    {
                                        shared.work.lock().unwrap().blocked[usize::from(key.dem)] =
                                            Some(Instant::now() + Duration::from_secs(60));
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Err(message) => {
                            if message.contains("429") || message.contains("503") {
                                shared.work.lock().unwrap().blocked[usize::from(key.dem)] =
                                    Some(Instant::now() + Duration::from_secs(60));
                            }
                            if tx.send_blocking((key, Err(message))).is_err() {
                                return;
                            }
                        }
                    }
                }
            });
        }
        drop(tx);
        let weak = Rc::downgrade(&ui);
        zgui::reactive::spawn_local(async move {
            while let Ok((key, value)) = rx.recv().await {
                let Some(ui) = weak.upgrade() else {
                    return;
                };
                let mut cache = ui.cache.borrow_mut();
                cache.tick += 1;
                let used = cache.tick;
                let value = value.map(|payload| {
                    let image = match payload.as_ref() {
                        Payload::Raster(r) => Some(zgui_image::ImageBytes::new(r.encoded.clone())),
                        _ => None,
                    };
                    Ready { payload, image }
                });
                cache.entries.insert(key, Entry { value, used });
                let mut work = ui.shared.work.lock().unwrap();
                work.claimed.remove(&key);
                trim(&mut cache, &work.desired);
                if memory(&cache) > MEMORY {
                    cache.entries.insert(
                        key,
                        Entry {
                            value: Err(
                                "Map tile memory budget reached; reduce the visible area".into()
                            ),
                            used,
                        },
                    );
                }
                work.known = cache.entries.keys().copied().collect();
                drop(work);
                drop(cache);
                ui.shared.changed.notify_all();
                revision.update(|r| *r = r.wrapping_add(1));
            }
        });
        on_cleanup_local(move || {
            shared.work.lock().unwrap().closed = true;
            shared.changed.notify_all();
        });
        Self {
            ui: StoredValue::new_local(ui),
            revision,
        }
    }

    pub fn demand(
        self,
        owner: u64,
        tiles: &[TileId],
        terrain: bool,
        provider: Provider,
        online: bool,
    ) {
        let elevation = if terrain { tiles } else { &[] };
        self.demand_layers(owner, tiles, elevation, provider, online);
    }
    pub fn demand_layers(
        self,
        owner: u64,
        raster: &[TileId],
        elevation: &[TileId],
        provider: Provider,
        online: bool,
    ) {
        let keys = raster
            .iter()
            .map(|&tile| Key::raster_from(tile, provider))
            .chain(elevation.iter().map(|&tile| Key::elevation(tile)))
            .collect::<Vec<_>>();
        let ui = self.ui.get_value();
        let mut cache = ui.cache.borrow_mut();
        let demand = Demand { keys, online };
        let became_online = online && cache.demands.get(&owner).is_some_and(|old| !old.online);
        if became_online {
            for key in &demand.keys {
                if cache
                    .entries
                    .get(key)
                    .is_some_and(|entry| entry.value.is_err())
                {
                    cache.entries.remove(key);
                }
            }
        }

        if cache.demands.get(&owner) == Some(&demand) {
            return;
        }
        if demand.keys.is_empty() {
            cache.demands.remove(&owner);
        } else {
            cache.demands.insert(owner, demand);
        }
        let priorities = cache.priorities();
        let desired = priorities.keys().copied().collect::<BTreeSet<_>>();
        cache.tick += 1;
        let tick = cache.tick;
        for key in &desired {
            if let Some(entry) = cache.entries.get_mut(key) {
                entry.used = tick;
            }
        }
        let mut work = ui.shared.work.lock().unwrap();
        work.desired = desired;
        work.priorities = priorities;
        work.online = cache
            .demands
            .values()
            .filter(|demand| demand.online)
            .flat_map(|demand| demand.keys.iter().copied())
            .collect();
        trim(&mut cache, &work.desired);
        work.known = cache.entries.keys().copied().collect();
        drop(work);
        drop(cache);
        ui.shared.changed.notify_all();
        self.revision.try_update(|r| *r = r.wrapping_add(1));
    }
    pub fn payload(self, key: Key) -> Option<Arc<Payload>> {
        self.revision.get();
        self.ui.with_value(|ui| {
            let mut cache = ui.cache.borrow_mut();
            cache.tick += 1;
            let tick = cache.tick;
            let entry = cache.entries.get_mut(&key)?;
            entry.used = tick;
            entry.value.as_ref().ok().map(|ready| ready.payload.clone())
        })
    }
    #[cfg(test)]
    pub fn cached_children(self, parent: TileId) -> Vec<TileId> {
        self.ui.with_value(|ui| {
            let cache = ui.cache.borrow();
            cache
                .entries
                .iter()
                .filter_map(|(key, entry)| {
                    if key.dem || key.z <= parent.z || entry.value.is_err() {
                        return None;
                    }
                    let shift = key.z - parent.z;
                    if key.x >> shift != parent.canonical_x() || key.y >> shift != parent.y {
                        return None;
                    }
                    let id = TileId {
                        z: key.z,
                        x: parent.x * (1_i64 << shift) + i64::from(key.x % (1_u32 << shift)),
                        y: key.y,
                    };
                    cache
                        .entries
                        .get(&Key::elevation(id))
                        .filter(|e| e.value.is_ok())
                        .map(|_| id)
                })
                .collect()
        })
    }
    pub fn cached_raster_children(self, parent: TileId, provider: Provider) -> Vec<TileId> {
        self.ui.with_value(|ui| {
            ui.cache
                .borrow()
                .entries
                .iter()
                .filter_map(|(key, entry)| {
                    if key.dem
                        || key.provider != provider
                        || key.z <= parent.z
                        || entry.value.is_err()
                    {
                        return None;
                    }
                    let shift = key.z - parent.z;
                    if key.x >> shift != parent.canonical_x() || key.y >> shift != parent.y {
                        return None;
                    }
                    Some(TileId {
                        z: key.z,
                        x: parent.x * (1_i64 << shift) + i64::from(key.x % (1_u32 << shift)),
                        y: key.y,
                    })
                })
                .collect()
        })
    }
    pub fn url(self, tile: TileId, provider: Provider) -> Option<String> {
        self.revision.get();
        self.ui.with_value(|ui| {
            ui.cache
                .borrow()
                .entries
                .get(&Key::raster_from(tile, provider))?
                .value
                .as_ref()
                .ok()?
                .image
                .as_ref()
                .map(|i| i.url())
        })
    }
    pub fn retry(self) {
        let ui = self.ui.get_value();
        let mut cache = ui.cache.borrow_mut();
        cache.entries.retain(|_, entry| entry.value.is_ok());
        let mut work = ui.shared.work.lock().unwrap();
        work.known = cache.entries.keys().copied().collect();
        drop(work);
        drop(cache);
        ui.shared.changed.notify_all();
        self.revision.update(|r| *r = r.wrapping_add(1));
    }
    pub fn status(self) -> String {
        self.revision.get();
        let ui = self.ui.get_value();
        let cache = ui.cache.borrow();
        let work = ui.shared.work.lock().unwrap();
        let ready = work
            .desired
            .iter()
            .filter(|k| cache.entries.get(k).is_some_and(|e| e.value.is_ok()))
            .count();
        let errors = work
            .desired
            .iter()
            .filter_map(|k| cache.entries.get(k)?.value.as_ref().err())
            .collect::<Vec<_>>();
        if let Some(error) = errors.first() {
            format!(
                "{ready}/{} tiles ready | {} errors: {error} (Retry tiles)",
                work.desired.len(),
                errors.len()
            )
        } else if work.desired.is_empty() {
            "Checkerboard: no tile requests".into()
        } else {
            format!("{ready}/{} tiles ready", work.desired.len())
        }
    }
}

fn memory(cache: &Cache) -> usize {
    cache
        .entries
        .values()
        .filter_map(|e| e.value.as_ref().ok())
        .map(|r| r.payload.bytes())
        .sum::<usize>()
}
fn trim(cache: &mut Cache, desired: &BTreeSet<Key>) {
    let mut bytes = memory(cache);
    while cache.entries.len() > 1024 || bytes > MEMORY {
        let oldest = cache
            .entries
            .iter()
            .filter(|(key, _)| !desired.contains(key))
            .min_by_key(|(_, e)| e.used)
            .map(|(key, _)| *key);
        let Some(key) = oldest else {
            break;
        };
        if let Some(entry) = cache.entries.remove(&key) {
            bytes = bytes.saturating_sub(entry.value.ok().map_or(0, |r| r.payload.bytes()));
        }
    }
}

#[derive(Serialize, Deserialize)]
struct Metadata {
    expires: u64,
    etag: Option<String>,
    modified: Option<String>,
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn read_limited(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BODY + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BODY {
        return Err("Tile exceeds 4 MiB limit".into());
    }
    Ok(bytes)
}
fn load(
    config: &Config,
    agent: &ureq::Agent,
    key: Key,
    online: bool,
) -> Result<(Payload, bool), String> {
    load_impl(config, agent, key, online, true)
}
fn load_impl(
    config: &Config,
    agent: &ureq::Agent,
    key: Key,
    online: bool,
    serve_stale: bool,
) -> Result<(Payload, bool), String> {
    if key.z > 19 || key.x >= 1 << key.z || key.y >= 1 << key.z {
        return Err("Invalid tile coordinate".into());
    }
    if let Some(local) = &config.local {
        return decode(key, read_limited(&local.join(key.relative()))?).map(|tile| (tile, false));
    }
    let path = config.cache.join(key.relative());
    let meta_path = path.with_extension("json");
    let meta = std::fs::read(&meta_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Metadata>(&bytes).ok());
    let cached = read_limited(&path).ok();
    if meta.as_ref().is_some_and(|m| m.expires > now())
        && let Some(bytes) = &cached
        && let Ok(tile) = decode(key, bytes.clone())
    {
        return Ok((tile, false));
    }
    if !config.network || !online {
        return cached
            .ok_or("Tile is not cached; network is disabled".into())
            .and_then(|b| decode(key, b))
            .map(|tile| (tile, false));
    }
    if serve_stale
        && let Some(bytes) = &cached
        && let Ok(tile) = decode(key, bytes.clone())
    {
        return Ok((tile, true));
    }
    let url = if key.dem {
        format!(
            "https://tiles.mapterhorn.com/{}/{}/{}.webp",
            key.z, key.x, key.y
        )
    } else {
        key.provider.url(key)?
    };
    let mut request = agent.get(&url).header(
        "User-Agent",
        concat!(
            "Enlumi-map-preview/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/Quick-Flash/enlumi)"
        ),
    );
    if cached.is_some()
        && let Some(meta) = &meta
    {
        if let Some(etag) = &meta.etag {
            request = request.header("If-None-Match", etag);
        }
        if let Some(modified) = &meta.modified {
            request = request.header("If-Modified-Since", modified);
        }
    }
    let mut response = match request.call() {
        Ok(r) => r,
        Err(e) => {
            return cached
                .ok_or_else(|| {
                    if matches!(
                        key.provider,
                        Provider::MapboxStreets | Provider::MapboxSatellite
                    ) {
                        "Mapbox tile request failed; check ENLUMI_MAPBOX_TOKEN".into()
                    } else {
                        format!("Tile request failed: {e}")
                    }
                })
                .and_then(|b| decode(key, b))
                .map(|tile| (tile, false));
        }
    };
    let header = |name| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let cache_control = header("cache-control").unwrap_or_default();
    let ttl = cache_control
        .split(',')
        .find_map(|s| s.trim().strip_prefix("max-age=")?.parse::<u64>().ok())
        .unwrap_or(7 * 86400);
    let metadata = Metadata {
        expires: now().saturating_add(ttl),
        etag: header("etag").or_else(|| meta.as_ref().and_then(|m| m.etag.clone())),
        modified: header("last-modified")
            .or_else(|| meta.as_ref().and_then(|m| m.modified.clone())),
    };
    let bytes = if response.status().as_u16() == 304 {
        cached.ok_or("304 without cached tile")?
    } else {
        response
            .body_mut()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .map_err(|e| e.to_string())?
    };
    let tile = decode(key, bytes.clone())?;
    if !cache_control.contains("no-store") {
        persist_cached_tile(
            &config.cache,
            &path,
            &bytes,
            &serde_json::to_vec(&metadata).map_err(|e| e.to_string())?,
            DISK,
        )?;
    }
    Ok((tile, false))
}
fn persist_cached_tile(
    cache: &Path,
    path: &Path,
    bytes: &[u8],
    metadata: &[u8],
    limit: u64,
) -> Result<bool, String> {
    static SIZES: OnceLock<Mutex<BTreeMap<PathBuf, u64>>> = OnceLock::new();
    let mut sizes = SIZES.get_or_init(Default::default).lock().unwrap();
    let used = sizes
        .entry(cache.to_path_buf())
        .or_insert_with(|| disk_bytes(cache));
    let meta_path = path.with_extension("json");
    let old = std::fs::metadata(path).map_or(0, |m| m.len())
        + std::fs::metadata(&meta_path).map_or(0, |m| m.len());
    let next = used
        .saturating_sub(old)
        .saturating_add((bytes.len() + metadata.len()) as u64);
    if next > limit {
        // Disk capacity must not discard an already downloaded/decoded tile.
        return Ok(false);
    }
    write_atomic(path, bytes)?;
    write_atomic(&meta_path, metadata)?;
    *used = next;
    Ok(true)
}
fn disk_bytes(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                disk_bytes(&e.path())
            } else {
                e.metadata().map_or(0, |m| m.len())
            }
        })
        .sum()
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("Invalid cache path")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn decode(key: Key, bytes: Vec<u8>) -> Result<Payload, String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(&bytes);
    let version = hasher.finish();
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let dimensions = reader.into_dimensions().map_err(|e| e.to_string())?;
    let expected = if key.dem { (512, 512) } else { (256, 256) };
    if dimensions != expected {
        return Err(format!(
            "Expected {}x{} tile, received {dimensions:?}",
            expected.0, expected.1
        ));
    }
    let image = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
    if key.dem {
        let rgb = image.to_rgb8();
        let heights = rgb
            .pixels()
            .map(|p| f32::from(p[0]) * 256. + f32::from(p[1]) + f32::from(p[2]) / 256. - 32768.)
            .collect();
        Ok(Payload::Elevation(Elevation {
            version,
            heights,
            width: 512,
            height: 512,
        }))
    } else {
        Ok(Payload::Raster(Raster {
            version,
            encoded: bytes,
            rgba: image.to_rgba8().into_raw(),
            width: 256,
            height: 256,
        }))
    }
}

#[cfg(test)]
mod priority_tests {
    use super::*;
    #[test]
    fn full_disk_cache_keeps_existing_tiles_and_allows_uncached_rendering() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.png");
        assert!(persist_cached_tile(dir.path(), &first, b"tile", b"meta", 8).unwrap());
        let second = dir.path().join("second.png");
        assert!(!persist_cached_tile(dir.path(), &second, b"tile", b"meta", 8).unwrap());
        assert_eq!(std::fs::read(&first).unwrap(), b"tile");
        assert!(!second.exists());
        assert!(persist_cached_tile(dir.path(), &first, b"new", b"meta", 8).unwrap());
    }
    #[test]
    fn bing_expansion_deduplicates_root_and_child_tiles() {
        let root = TileId { z: 0, x: 0, y: 0 };
        let child = TileId { z: 1, x: 0, y: 0 };
        for provider in [
            Provider::BingRoad,
            Provider::BingSatellite,
            Provider::BingHybrid,
        ] {
            let tiles = provider.expand_tiles(vec![root, child, root]);
            assert_eq!(tiles.len(), 4);
            assert_eq!(tiles.iter().copied().collect::<BTreeSet<_>>().len(), 4);
            assert!(tiles.iter().all(|tile| tile.z == 1));
        }
    }
    #[test]
    fn workers_start_at_the_center_and_skip_claimed_or_blocked_tiles() {
        let middle = Key {
            dem: false,
            provider: Provider::OpenStreetMap,
            z: 12,
            x: 2100,
            y: 1400,
        };
        let corner = Key {
            x: 2099,
            y: 1399,
            ..middle
        };
        let dem = Key {
            dem: true,
            ..middle
        };
        let mut cache = Cache::default();
        cache.demands.insert(
            1,
            Demand {
                keys: vec![middle, dem, corner],
                online: true,
            },
        );
        cache.demands.insert(
            2,
            Demand {
                keys: vec![corner, middle],
                online: true,
            },
        );
        let priorities = cache.priorities();
        assert_eq!(priorities[&middle], 0);
        assert_eq!(priorities[&corner], 0);
        cache.demands.remove(&2);
        let priorities = cache.priorities();
        let mut work = Work {
            desired: priorities.keys().copied().collect(),
            priorities,
            ..Default::default()
        };
        let now = Instant::now();
        assert_eq!(work.next(0, now), Some(middle));
        assert_eq!(work.next(1, now), Some(dem));
        work.claimed.insert(middle);
        assert_eq!(work.next(0, now), Some(corner));
        work.blocked[0] = Some(now + Duration::from_secs(60));
        assert_eq!(work.next(0, now), Some(dem));
        work.known.insert(dem);
        assert_eq!(work.next(0, now), None);
    }
}

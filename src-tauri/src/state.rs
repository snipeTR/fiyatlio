// src-tauri/src/state.rs
//
// Managed application state plus its on-disk persistence.
//
// Everything the UI can see is derived from four things: the imported trades,
// the import summary that produced them, the user's settings, and the most
// recent price snapshot. Holding them in one guarded struct means a settings
// change and a price refresh can never observe a half-updated world.
//
// Concurrency rule: a `std::sync::Mutex` guard must NEVER be held across an
// `.await`. Async commands clone what they need, drop the guard, do the I/O, then
// re-acquire to write back.

use std::sync::Mutex;

use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use crate::binance::PriceClient;
use crate::error::{AppError, AppResult};
use crate::models::{ImportSummary, PriceSnapshot, Settings, Trade};

/// Single JSON file backing both settings and the price cache.
pub const STORE_FILE: &str = "fiyatlio.json";

const KEY_SETTINGS: &str = "settings";
const KEY_PRICES: &str = "priceSnapshot";

/// The mutable half of the application state.
pub struct AppData {
    pub trades: Vec<Trade>,
    pub import: ImportSummary,
    pub settings: Settings,
    /// Last known prices. `None` only before the very first successful fetch on
    /// a machine that has never been online.
    pub prices: Option<PriceSnapshot>,
}

impl Default for AppData {
    fn default() -> Self {
        Self {
            trades: Vec::new(),
            import: ImportSummary::empty(),
            settings: Settings::default(),
            prices: None,
        }
    }
}

/// Tauri-managed state. The HTTP client lives here too so connections are pooled
/// across refreshes instead of being rebuilt per request.
pub struct AppState {
    data: Mutex<AppData>,
    pub client: PriceClient,
}

impl AppState {
    pub fn new(client: PriceClient) -> Self {
        Self {
            data: Mutex::new(AppData::default()),
            client,
        }
    }

    /// Runs `f` under the lock.
    ///
    /// A poisoned mutex means another thread panicked mid-update. Rather than
    /// propagating the panic we surface it as a normal error — the UI shows a
    /// message and the user can re-import, which is strictly better than taking
    /// the process down.
    pub fn with<T>(&self, f: impl FnOnce(&mut AppData) -> T) -> AppResult<T> {
        let mut guard = self
            .data
            .lock()
            .map_err(|_| AppError::Internal("application state lock was poisoned".into()))?;
        Ok(f(&mut guard))
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Reads settings from disk, falling back to defaults.
///
/// A corrupt or partially written store must not block startup: anything that
/// fails to deserialize is logged and replaced with the default, because an app
/// that refuses to launch over a bad preference file is worse than one that
/// forgets a preference.
pub fn load_settings<R: Runtime>(app: &AppHandle<R>) -> Settings {
    let mut settings = app
        .store(STORE_FILE)
        .ok()
        .and_then(|store| store.get(KEY_SETTINGS))
        .and_then(|value| match serde_json::from_value::<Settings>(value) {
            Ok(settings) => Some(settings),
            Err(e) => {
                tracing::warn!("stored settings could not be read, using defaults: {e}");
                None
            }
        })
        .unwrap_or_default();

    settings.sanitize();
    settings
}

pub fn save_settings<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> AppResult<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::Store(e.to_string()))?;

    let value = serde_json::to_value(settings).map_err(|e| AppError::Store(e.to_string()))?;
    store.set(KEY_SETTINGS, value);
    store.save().map_err(|e| AppError::Store(e.to_string()))
}

/// Reads the cached price snapshot, always marked stale.
///
/// Anything restored from disk is by definition not live, so the UI shows the
/// "as of …" indicator until a fresh fetch succeeds.
pub fn load_prices<R: Runtime>(app: &AppHandle<R>) -> Option<PriceSnapshot> {
    let value = app.store(STORE_FILE).ok()?.get(KEY_PRICES)?;

    match serde_json::from_value::<PriceSnapshot>(value) {
        Ok(mut snapshot) => {
            snapshot.stale = true;
            Some(snapshot)
        }
        Err(e) => {
            tracing::warn!("cached prices could not be read: {e}");
            None
        }
    }
}

pub fn save_prices<R: Runtime>(app: &AppHandle<R>, snapshot: &PriceSnapshot) -> AppResult<()> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| AppError::Store(e.to_string()))?;

    let value = serde_json::to_value(snapshot).map_err(|e| AppError::Store(e.to_string()))?;
    store.set(KEY_PRICES, value);
    store.save().map_err(|e| AppError::Store(e.to_string()))
}

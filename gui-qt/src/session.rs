//! `SessionManager` QObject: the entire Qt-facing backend.
//!
//! Long/slow work (BLE scans, connects) runs on worker threads and pushes
//! results back via `CxxQtThread::queue`. Fast sqlite/state reads block
//! briefly via a shared tokio runtime. All QML payloads are JSON built by
//! [`crate::model`] (pure, unit-tested).

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        /// An alias to the QString type
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        /// Backend exposed to QML as `SessionManager {}` after
        /// `import com.openscq30.guiqt 1.0`.
        #[qobject]
        #[qml_element]
        #[qproperty(QString, status)]
        #[qproperty(QString, error_message)]
        #[qproperty(QString, paired_devices_json)]
        #[qproperty(i32, paired_count)]
        #[qproperty(QString, device_models_json)]
        #[qproperty(QString, scan_status)]
        #[qproperty(QString, scan_results_json)]
        #[qproperty(QString, connected_mac)]
        #[qproperty(QString, connected_model_name)]
        #[qproperty(QString, connection_status)]
        #[qproperty(QString, categories_json)]
        #[qproperty(QString, active_category)]
        #[qproperty(QString, settings_json)]
        #[qproperty(i32, settings_version)]
        #[qproperty(QString, quick_presets_json)]
        #[qproperty(QString, legacy_profiles_json)]
        type SessionManager = super::SessionManagerRust;

        /// Reload the paired-device list from `database.sqlite`.
        #[qinvokable]
        #[cxx_name = "refreshPairedDevices"]
        fn refresh_paired_devices(self: Pin<&mut SessionManager>);

        /// Load `[{id, name}]` of all device models for the add-device picker.
        #[qinvokable]
        #[cxx_name = "loadDeviceModels"]
        fn load_device_models(self: Pin<&mut SessionManager>);

        /// Bluetooth scan on a worker thread.
        #[qinvokable]
        #[cxx_name = "startScan"]
        fn start_scan(self: Pin<&mut SessionManager>, model_id: &QString);

        /// Persist a mac/model association, then refresh the paired list.
        #[qinvokable]
        #[cxx_name = "pairDevice"]
        fn pair_device(
            self: Pin<&mut SessionManager>,
            mac: &QString,
            model_id: &QString,
        );

        /// Remove a pairing, then refresh the paired list.
        #[qinvokable]
        #[cxx_name = "unpairDevice"]
        fn unpair_device(self: Pin<&mut SessionManager>, mac: &QString);

        /// Connect on a worker thread; UI updates via queued closure.
        #[qinvokable]
        #[cxx_name = "connectToDevice"]
        fn connect_to_device(self: Pin<&mut SessionManager>, mac: &QString);

        /// Drop the device, obsolete its watcher, reset connection properties.
        #[qinvokable]
        #[cxx_name = "disconnectDevice"]
        fn disconnect_device(self: Pin<&mut SessionManager>);

        /// Switch the visible settings category.
        #[qinvokable]
        #[cxx_name = "selectCategory"]
        fn select_category(self: Pin<&mut SessionManager>, category_id: &QString);

        /// Re-read the active category from the connected device.
        #[qinvokable]
        #[cxx_name = "refreshSettings"]
        fn refresh_settings(self: Pin<&mut SessionManager>);

        /// Set one setting. `value_json` is `{type, value}` (see `model` docs).
        #[qinvokable]
        #[cxx_name = "setSettingValue"]
        fn set_setting_value(
            self: Pin<&mut SessionManager>,
            setting_id: &QString,
            value_json: &QString,
        );

        /// Reload quick presets of the connected device.
        #[qinvokable]
        #[cxx_name = "loadQuickPresets"]
        fn load_quick_presets(self: Pin<&mut SessionManager>);

        /// Snapshot current settings into a new preset (fields start disabled).
        #[qinvokable]
        #[cxx_name = "createQuickPreset"]
        fn create_quick_preset(self: Pin<&mut SessionManager>, name: &QString);

        /// Delete a preset.
        #[qinvokable]
        #[cxx_name = "deleteQuickPreset"]
        fn delete_quick_preset(self: Pin<&mut SessionManager>, name: &QString);

        /// Apply all enabled fields of a preset, then refresh settings.
        #[qinvokable]
        #[cxx_name = "activateQuickPreset"]
        fn activate_quick_preset(self: Pin<&mut SessionManager>, name: &QString);

        /// Enable/disable one preset field.
        #[qinvokable]
        #[cxx_name = "togglePresetField"]
        fn toggle_preset_field(
            self: Pin<&mut SessionManager>,
            name: &QString,
            setting_id: &QString,
            is_enabled: bool,
        );

        /// Overwrite a preset's values with the current settings.
        #[qinvokable]
        #[cxx_name = "snapshotPreset"]
        fn snapshot_preset(self: Pin<&mut SessionManager>, name: &QString);

        /// Load v1 legacy EQ profiles from `config_dir/config.toml`.
        #[qinvokable]
        #[cxx_name = "loadLegacyProfiles"]
        fn load_legacy_profiles(self: Pin<&mut SessionManager>);

        /// Import one v1 profile as a custom EQ profile.
        #[qinvokable]
        #[cxx_name = "migrateLegacyProfile"]
        fn migrate_legacy_profile(self: Pin<&mut SessionManager>, name: &QString);
    }

    impl cxx_qt::Threading for SessionManager {}
}

use core::pin::Pin;
use std::sync::{
    atomic::{AtomicI32, AtomicU64, Ordering},
    Mutex,
};

use cxx_qt::{CxxQtThread, Threading};
use cxx_qt_lib::QString;
use openscq30_i18n::Translate;
use openscq30_lib::{
    connection::ConnectionStatus, device::OpenSCQ30Device, settings::CategoryId,
    storage::PairedDevice, OpenSCQ30Session,
};
use std::sync::Arc;

use crate::model;

/// The Rust struct backing the `SessionManager` QObject.
///
/// Only `#[qproperty]` fields live here (accessed via generated accessors).
/// Shared backend state (runtime, connection, watcher generations) is
/// process-global because QObjects must stay movable across Qt/C++.
#[derive(Default)]
pub struct SessionManagerRust {
    status: QString,
    error_message: QString,
    paired_devices_json: QString,
    paired_count: i32,
    device_models_json: QString,
    scan_status: QString,
    scan_results_json: QString,
    connected_mac: QString,
    connected_model_name: QString,
    connection_status: QString,
    categories_json: QString,
    active_category: QString,
    settings_json: QString,
    settings_version: i32,
    quick_presets_json: QString,
    legacy_profiles_json: QString,
}

static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
static CONNECTED: Mutex<Option<Arc<dyn OpenSCQ30Device + Send + Sync>>> = Mutex::new(None);
static ACTIVE_CATEGORY: Mutex<Option<CategoryId>> = Mutex::new(None);
/// Bumped on every (re)connect, scan and disconnect; stale worker threads and
/// watchers check this and exit without touching the UI.
static CONNECT_GEN: AtomicU64 = AtomicU64::new(0);
static SCAN_GEN: AtomicU64 = AtomicU64::new(0);
static SETTINGS_VERSION: AtomicI32 = AtomicI32::new(0);

fn runtime() -> anyhow::Result<&'static tokio::runtime::Runtime> {
    if let Some(rt) = RUNTIME.get() {
        return Ok(rt);
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    Ok(RUNTIME.get_or_init(|| rt))
}

fn open_session() -> anyhow::Result<OpenSCQ30Session> {
    let path = model::default_db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(runtime()?.block_on(OpenSCQ30Session::new(path))?)
}

fn config_dir_path() -> std::path::PathBuf {
    model::config_path()
        .parent()
        .map_or_else(std::env::temp_dir, std::path::Path::to_path_buf)
}

fn connected_device() -> anyhow::Result<Arc<dyn OpenSCQ30Device + Send + Sync>> {
    CONNECTED
        .lock()
        .unwrap()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("no device connected"))
}

fn preset_handler() -> anyhow::Result<openscq30_lib::quick_presets::QuickPresetsHandler> {
    Ok(open_session()?.quick_preset_handler())
}

/// Publish the connected device's presets, or the error.
fn push_quick_presets(qt: Pin<&mut qobject::SessionManager>) {
    let mut qt = qt;
    let outcome: anyhow::Result<String> = (|| {
        let device = connected_device()?;
        let handler = preset_handler()?;
        let presets = runtime()?.block_on(handler.quick_presets(&*device))?;
        Ok(model::quick_presets_json(&*device, &presets))
    })();
    match outcome {
        Ok(json) => qt.as_mut().set_quick_presets_json(QString::from(&json)),
        Err(err) => qt
            .as_mut()
            .set_error_message(QString::from(&err.to_string())),
    }
}

struct ConnectedInfo {
    generation: u64,
    mac: String,
    model_name: String,
    categories_json: String,
    first_category: Option<CategoryId>,
}

fn connect_blocking(mac_str: &str) -> anyhow::Result<ConnectedInfo> {
    let mac = model::parse_mac(mac_str)?;
    let session = open_session()?;
    let device = runtime()?.block_on(session.connect(mac))?;
    let categories = device.categories();
    let info = ConnectedInfo {
        generation: CONNECT_GEN.fetch_add(1, Ordering::SeqCst) + 1,
        mac: mac_str.to_owned(),
        model_name: device.model().translate(),
        categories_json: model::categories_json(&categories),
        first_category: categories.into_iter().next(),
    };
    *CONNECTED.lock().unwrap() = Some(device);
    Ok(info)
}

fn scan_blocking(model_str: &str) -> anyhow::Result<(String, i32)> {
    let model = model::parse_model(model_str)?;
    let session = open_session()?;
    let descriptors = runtime()?.block_on(session.list_devices(model))?;
    Ok(model::scan_results_json(&descriptors))
}

/// Re-read the active category from the connected device and publish it.
/// Called from the Qt thread (invokables and queued watcher closures).
fn push_current_settings(qt: Pin<&mut qobject::SessionManager>) {
    let device = CONNECTED.lock().unwrap().clone();
    let category = *ACTIVE_CATEGORY.lock().unwrap();
    if let (Some(device), Some(category)) = (device, category) {
        let mut qt = qt;
        qt.as_mut()
            .set_settings_json(QString::from(&model::settings_json(&*device, &category)));
        let version = SETTINGS_VERSION.fetch_add(1, Ordering::SeqCst) + 1;
        qt.as_mut().set_settings_version(version);
    }
}

/// Background thread forwarding device changes to QML. Exits when its
/// generation is obsoleted (reconnect/disconnect) or the device is dropped.
fn spawn_watcher(qt_thread: CxxQtThread<qobject::SessionManager>, generation: u64) {
    std::thread::spawn(move || {
        let runtime = match runtime() {
            Ok(rt) => rt,
            Err(_) => return,
        };
        let (mut changes, mut status) = match CONNECTED.lock().unwrap().clone() {
            Some(device) => (device.watch_for_changes(), device.connection_status()),
            None => return,
        };
        runtime.block_on(async move {
            loop {
                if CONNECT_GEN.load(Ordering::SeqCst) != generation {
                    break;
                }
                tokio::select! {
                    result = changes.changed() => {
                        if result.is_err() {
                            break;
                        }
                        if CONNECT_GEN.load(Ordering::SeqCst) != generation {
                            break;
                        }
                        let _ = qt_thread.queue(push_current_settings);
                    }
                    result = status.changed() => {
                        if result.is_err() {
                            break;
                        }
                        if *status.borrow() == ConnectionStatus::Disconnected {
                            let _ = qt_thread.queue(|mut q| {
                                q.as_mut().set_connection_status(QString::from("Disconnected"));
                                q.as_mut().set_connected_mac(QString::from(""));
                            });
                            break;
                        }
                    }
                }
            }
        });
    });
}

impl qobject::SessionManager {
    pub fn refresh_paired_devices(mut self: Pin<&mut Self>) {
        self.as_mut().set_status(QString::from("Loading..."));
        self.as_mut().set_error_message(QString::from(""));
        match (|| -> anyhow::Result<(String, i32)> {
            let session = open_session()?;
            let paired = runtime()?.block_on(session.paired_devices())?;
            Ok(model::paired_devices_json(&paired))
        })() {
            Ok((json, count)) => {
                self.as_mut().set_paired_devices_json(QString::from(&json));
                self.as_mut().set_paired_count(count);
                self.as_mut()
                    .set_status(QString::from(&format!("Ready - {count} paired device(s)")));
            }
            Err(err) => {
                self.as_mut()
                    .set_status(QString::from("Failed to load paired devices"));
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn load_device_models(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_device_models_json(QString::from(&model::device_models_json()));
    }

    pub fn start_scan(mut self: Pin<&mut Self>, model_id: &QString) {
        let model_str = model_id.to_string();
        self.as_mut().set_scan_status(QString::from("Scanning..."));
        self.as_mut().set_scan_results_json(QString::from("[]"));
        let generation = SCAN_GEN.fetch_add(1, Ordering::SeqCst) + 1;
        let qt_thread = self.qt_thread();
        std::thread::spawn(move || {
            let outcome = scan_blocking(&model_str);
            let _ = qt_thread.queue(move |mut q| {
                if SCAN_GEN.load(Ordering::SeqCst) != generation {
                    return;
                }
                match outcome {
                    Ok((json, count)) => {
                        q.as_mut().set_scan_results_json(QString::from(&json));
                        q.as_mut()
                            .set_scan_status(QString::from(&format!("Found {count} device(s)")));
                    }
                    Err(err) => {
                        q.as_mut()
                            .set_scan_status(QString::from(&format!("Scan failed: {err}")));
                    }
                }
            });
        });
    }

    pub fn pair_device(mut self: Pin<&mut Self>, mac: &QString, model_id: &QString) {
        let outcome: anyhow::Result<String> = (|| {
            let mac = model::parse_mac(&mac.to_string())?;
            let model = model::parse_model(&model_id.to_string())?;
            let session = open_session()?;
            runtime()?.block_on(session.pair(PairedDevice {
                mac_address: mac,
                model,
                is_demo: false,
            }))?;
            Ok(format!("Paired {mac}"))
        })();
        match outcome {
            Ok(message) => {
                self.as_mut().refresh_paired_devices();
                self.as_mut().set_status(QString::from(&message));
            }
            Err(err) => {
                self.as_mut().set_status(QString::from("Pairing failed"));
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn unpair_device(mut self: Pin<&mut Self>, mac: &QString) {
        let outcome: anyhow::Result<String> = (|| {
            let mac = model::parse_mac(&mac.to_string())?;
            let session = open_session()?;
            runtime()?.block_on(session.unpair(mac))?;
            Ok(format!("Removed {mac}"))
        })();
        match outcome {
            Ok(message) => {
                self.as_mut().refresh_paired_devices();
                self.as_mut().set_status(QString::from(&message));
            }
            Err(err) => {
                self.as_mut().set_status(QString::from("Removal failed"));
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn connect_to_device(mut self: Pin<&mut Self>, mac: &QString) {
        let mac_str = mac.to_string();
        self.as_mut()
            .set_connection_status(QString::from("Connecting..."));
        self.as_mut().set_error_message(QString::from(""));
        let qt_thread = self.qt_thread();
        let watcher_thread = qt_thread.clone();
        std::thread::spawn(move || {
            let outcome = connect_blocking(&mac_str);
            let _ = qt_thread.queue(move |mut q| match outcome {
                Ok(info) => {
                    if CONNECT_GEN.load(Ordering::SeqCst) != info.generation {
                        return;
                    }
                    q.as_mut().set_connected_mac(QString::from(&info.mac));
                    q.as_mut()
                        .set_connected_model_name(QString::from(&info.model_name));
                    q.as_mut().set_connection_status(QString::from("Connected"));
                    q.as_mut()
                        .set_categories_json(QString::from(&info.categories_json));
                    q.as_mut().set_error_message(QString::from(""));
                    if let Some(first) = info.first_category {
                        *ACTIVE_CATEGORY.lock().unwrap() = Some(first);
                        q.as_mut()
                            .set_active_category(QString::from(&first.to_string()));
                    }
                    push_current_settings(q);
                    spawn_watcher(watcher_thread, info.generation);
                }
                Err(err) => {
                    q.as_mut().set_connection_status(QString::from("Failed"));
                    q.as_mut()
                        .set_error_message(QString::from(&err.to_string()));
                }
            });
        });
    }

    pub fn disconnect_device(mut self: Pin<&mut Self>) {
        CONNECT_GEN.fetch_add(1, Ordering::SeqCst);
        *CONNECTED.lock().unwrap() = None;
        *ACTIVE_CATEGORY.lock().unwrap() = None;
        self.as_mut()
            .set_connection_status(QString::from("Disconnected"));
        self.as_mut().set_connected_mac(QString::from(""));
        self.as_mut().set_connected_model_name(QString::from(""));
        self.as_mut().set_categories_json(QString::from("[]"));
        self.as_mut().set_active_category(QString::from(""));
        self.as_mut().set_settings_json(QString::from("[]"));
    }

    pub fn select_category(mut self: Pin<&mut Self>, category_id: &QString) {
        let outcome: anyhow::Result<CategoryId> = (|| {
            let id = model::parse_category_id(&category_id.to_string())?;
            let device = CONNECTED
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| anyhow::anyhow!("no device connected"))?;
            if !device.categories().contains(&id) {
                anyhow::bail!("category {id} is not available on this device");
            }
            Ok(id)
        })();
        match outcome {
            Ok(id) => {
                *ACTIVE_CATEGORY.lock().unwrap() = Some(id);
                self.as_mut()
                    .set_active_category(QString::from(&id.to_string()));
                self.as_mut().set_error_message(QString::from(""));
                push_current_settings(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn refresh_settings(self: Pin<&mut Self>) {
        push_current_settings(self);
    }

    pub fn set_setting_value(mut self: Pin<&mut Self>, setting_id: &QString, value_json: &QString) {
        let outcome: anyhow::Result<()> = (|| {
            let id = model::parse_setting_id(&setting_id.to_string())?;
            let value = model::parse_value(&value_json.to_string())?;
            let device = CONNECTED
                .lock()
                .unwrap()
                .clone()
                .ok_or_else(|| anyhow::anyhow!("no device connected"))?;
            runtime()?.block_on(device.set_setting_values(vec![(id, value)]))?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                self.as_mut().set_error_message(QString::from(""));
                push_current_settings(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn load_quick_presets(self: Pin<&mut Self>) {
        push_quick_presets(self);
    }

    pub fn create_quick_preset(mut self: Pin<&mut Self>, name: &QString) {
        let outcome: anyhow::Result<()> = (|| {
            let name = name.to_string();
            if name.trim().is_empty() {
                anyhow::bail!("preset name must not be empty");
            }
            let device = connected_device()?;
            let handler = preset_handler()?;
            runtime()?.block_on(handler.save(&*device, name))?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                self.as_mut().set_error_message(QString::from(""));
                push_quick_presets(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn delete_quick_preset(mut self: Pin<&mut Self>, name: &QString) {
        let outcome: anyhow::Result<()> = (|| {
            let device = connected_device()?;
            let handler = preset_handler()?;
            runtime()?.block_on(handler.delete(&*device, name.to_string()))?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                self.as_mut().set_error_message(QString::from(""));
                push_quick_presets(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn activate_quick_preset(mut self: Pin<&mut Self>, name: &QString) {
        let outcome: anyhow::Result<()> = (|| {
            let device = connected_device()?;
            let handler = preset_handler()?;
            runtime()?.block_on(handler.activate(&*device, name.to_string()))?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                self.as_mut().set_error_message(QString::from(""));
                push_current_settings(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn toggle_preset_field(
        mut self: Pin<&mut Self>,
        name: &QString,
        setting_id: &QString,
        is_enabled: bool,
    ) {
        let outcome: anyhow::Result<()> = (|| {
            let setting_id = model::parse_setting_id(&setting_id.to_string())?;
            let device = connected_device()?;
            let handler = preset_handler()?;
            runtime()?.block_on(handler.toggle_field(
                &*device,
                name.to_string(),
                setting_id,
                is_enabled,
            ))?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                push_quick_presets(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn snapshot_preset(mut self: Pin<&mut Self>, name: &QString) {
        let outcome: anyhow::Result<()> = (|| {
            let device = connected_device()?;
            let handler = preset_handler()?;
            runtime()?.block_on(handler.save(&*device, name.to_string()))?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                self.as_mut().set_error_message(QString::from(""));
                push_quick_presets(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }

    pub fn load_legacy_profiles(mut self: Pin<&mut Self>) {
        self.as_mut()
            .set_legacy_profiles_json(QString::from(&model::legacy_profiles_json(
                &config_dir_path(),
            )));
    }

    pub fn migrate_legacy_profile(mut self: Pin<&mut Self>, name: &QString) {
        let outcome: anyhow::Result<()> = (|| {
            let name = name.to_string();
            let values = model::legacy_profile_values(&config_dir_path(), &name)
                .ok_or_else(|| anyhow::anyhow!("profile {name:?} not found"))?;
            let device = connected_device()?;
            runtime()?.block_on(model::migrate_legacy_profile(&*device, &name, values))?;
            Ok(())
        })();
        match outcome {
            Ok(()) => {
                // The migrated profile disappears from the legacy list: that is
                // the success feedback.
                self.as_mut().set_error_message(QString::from(""));
                self.as_mut().load_legacy_profiles();
                push_current_settings(self);
            }
            Err(err) => {
                self.as_mut()
                    .set_error_message(QString::from(&err.to_string()));
            }
        }
    }
}

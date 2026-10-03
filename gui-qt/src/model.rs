//! Pure (Qt-free) logic for the Qt GUI: JSON builders for QML and input parsing.
//!
//! Everything here is deterministic and unit-tested below. [`session`](super::session)
//! is a thin QObject wrapper around these helpers.

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use macaddr::MacAddr6;
use openscq30_i18n::Translate;
use openscq30_lib::{
    connection::ConnectionDescriptor,
    device::OpenSCQ30Device,
    settings::{localize_value, CategoryId, Setting, SettingId, Value},
    storage::PairedDevice,
    DeviceModel,
};
use serde_json::json;
use strum::VariantArray;

/// Config-dir sqlite path shared with the COSMIC GUI and CLI.
pub fn default_db_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    base.join("openscq30").join("database.sqlite")
}

/// `(json, count)` for the paired-device list.
/// `[{mac, model, modelName, isDemo}]`.
pub fn paired_devices_json(paired: &[PairedDevice]) -> (String, i32) {
    let entries: Vec<serde_json::Value> = paired
        .iter()
        .map(|d| {
            json!({
                "mac": d.mac_address.to_string(),
                "model": d.model.to_string(),
                "modelName": d.model.translate(),
                "isDemo": d.is_demo,
            })
        })
        .collect();
    let count = entries.len() as i32;
    (
        serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into()),
        count,
    )
}

/// `[{id, name}]` for the add-device model picker, sorted by id like the CLI.
pub fn device_models_json() -> String {
    let mut models = DeviceModel::VARIANTS.to_vec();
    models.sort_by_key(|model| model.to_string());
    let entries: Vec<serde_json::Value> = models
        .iter()
        .map(|model| {
            json!({
                "id": model.to_string(),
                "name": model.translate(),
            })
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
}

/// `[{name, macAddress}]` for scan results (`ConnectionDescriptor` shape).
pub fn scan_results_json(descriptors: &[ConnectionDescriptor]) -> (String, i32) {
    let count = descriptors.len() as i32;
    (
        serde_json::to_string(descriptors).unwrap_or_else(|_| "[]".into()),
        count,
    )
}

/// `[{id, name}]` for the category selector.
pub fn categories_json(ids: &[CategoryId]) -> String {
    let entries: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            json!({
                "id": id.to_string(),
                "name": id.translate(),
            })
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
}

/// Settings of one category, in device order. See [`setting_entry`] for the shape.
pub fn settings_json(device: &dyn OpenSCQ30Device, category: &CategoryId) -> String {
    let pairs: Vec<(SettingId, Setting)> = device
        .settings_in_category(category)
        .into_iter()
        .filter_map(|id| device.setting(&id).map(|setting| (id, setting)))
        .collect();
    settings_json_for_pairs(pairs)
}

/// Pure part of [`settings_json`], directly unit-testable without a device.
pub fn settings_json_for_pairs(pairs: Vec<(SettingId, Setting)>) -> String {
    let entries: Vec<serde_json::Value> = pairs
        .iter()
        .map(|(id, setting)| setting_entry(id, setting))
        .collect();
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
}

/// One settings row. Common keys: `id`, `name`, `mode`
/// (`readWrite`|`readOnly`|`writeOnly`), `displayValue` (localized current
/// value), `kind`. Kind-specific keys are documented per branch; QML builds
/// `Value` JSON (`{type, value}`) from them when writing back.
fn setting_entry(id: &SettingId, setting: &Setting) -> serde_json::Value {
    let mode = match setting.mode() {
        openscq30_lib::settings::SettingMode::ReadWrite => "readWrite",
        openscq30_lib::settings::SettingMode::ReadOnly => "readOnly",
        openscq30_lib::settings::SettingMode::WriteOnly => "writeOnly",
    };
    let display_value = localize_value(Some(setting), &Value::from(setting.clone()));
    let mut entry = json!({
        "id": id.to_string(),
        "name": id.translate(),
        "mode": mode,
        "displayValue": display_value,
    });
    let kind: &str = match setting {
        Setting::Toggle { value } => {
            entry["value"] = json!(value);
            "toggle"
        }
        Setting::I32Range { setting, value } => {
            // Extracted explicitly so QML never depends on serde's
            // `RangeInclusive` representation.
            entry["min"] = json!(setting.range.start());
            entry["max"] = json!(setting.range.end());
            entry["step"] = json!(setting.step);
            entry["value"] = json!(value);
            "range"
        }
        Setting::Select { setting, value } => {
            entry["options"] = json!(setting.options);
            entry["localizedOptions"] = json!(setting.localized_options);
            entry["value"] = json!(value);
            entry["allowNone"] = json!(false);
            entry["modifiable"] = json!(false);
            "select"
        }
        Setting::OptionalSelect { setting, value } => {
            entry["options"] = json!(setting.options);
            entry["localizedOptions"] = json!(setting.localized_options);
            entry["value"] = json!(value);
            entry["allowNone"] = json!(true);
            entry["modifiable"] = json!(false);
            "select"
        }
        Setting::ModifiableSelect { setting, value } => {
            entry["options"] = json!(setting.options);
            entry["localizedOptions"] = json!(setting.localized_options);
            entry["value"] = json!(value);
            entry["allowNone"] = json!(true);
            entry["modifiable"] = json!(true);
            "select"
        }
        Setting::MultiSelect { setting, values } => {
            entry["options"] = json!(setting.options);
            entry["localizedOptions"] = json!(setting.localized_options);
            entry["values"] = json!(values);
            entry["removable"] = json!(false);
            "multi"
        }
        Setting::MultiSelectWithRemove { setting, values } => {
            entry["options"] = json!(setting.options);
            entry["localizedOptions"] = json!(setting.localized_options);
            entry["values"] = json!(values);
            entry["removable"] = json!(true);
            "multi"
        }
        Setting::Equalizer { setting, value, .. } => {
            entry["bandHz"] = json!(setting.band_hz);
            entry["fractionDigits"] = json!(setting.fraction_digits);
            entry["min"] = json!(setting.min);
            entry["max"] = json!(setting.max);
            entry["value"] = json!(value);
            "equalizer"
        }
        Setting::PresetEqualizerProfileSelect {
            equalizer,
            select,
            value,
            ..
        } => {
            entry["options"] = json!(select.options);
            entry["localizedOptions"] = json!(select.localized_options);
            entry["value"] = json!(value);
            entry["bandHz"] = json!(equalizer.band_hz);
            entry["min"] = json!(equalizer.min);
            entry["max"] = json!(equalizer.max);
            "presetEqualizer"
        }
        Setting::Information { .. } => "info",
        Setting::ImportString {
            confirmation_message,
        } => {
            entry["hint"] = json!(confirmation_message);
            "import"
        }
        Setting::HueColorPicker { hue } => {
            entry["value"] = json!(hue);
            "hue"
        }
        Setting::Action => "action",
        Setting::TimeOfDay {
            minutes_after_midnight,
        } => {
            entry["value"] = json!(minutes_after_midnight);
            "time"
        }
    };
    entry["kind"] = json!(kind);
    entry
}

pub fn parse_mac(s: &str) -> anyhow::Result<MacAddr6> {
    s.parse()
        .map_err(|err| anyhow::anyhow!("invalid MAC address {s:?}: {err}"))
}

pub fn parse_model(s: &str) -> anyhow::Result<DeviceModel> {
    DeviceModel::from_str(s).map_err(|_| anyhow::anyhow!("unknown device model {s:?}"))
}

pub fn parse_setting_id(s: &str) -> anyhow::Result<SettingId> {
    SettingId::from_str(s).map_err(|_| anyhow::anyhow!("unknown setting id {s:?}"))
}

pub fn parse_category_id(s: &str) -> anyhow::Result<CategoryId> {
    CategoryId::from_str(s).map_err(|_| anyhow::anyhow!("unknown category id {s:?}"))
}

/// Parses a `Value` from the `{type, value}` JSON that QML builds.
pub fn parse_value(json_str: &str) -> anyhow::Result<Value> {
    serde_json::from_str(json_str)
        .map_err(|err| anyhow::anyhow!("invalid setting value {json_str:?}: {err}"))
}

// --- Config / language -----------------------------------------------------
//
// No picker UI: the stored preference (shared with the COSMIC GUI config
// file) or, normally, plain system detection decides. UI chrome itself is
// not localized yet.

/// Mirrors the COSMIC GUI's config file so both UIs share preferences.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct GuiConfig {
    preferred_language: Option<String>,
}

/// `config_dir/openscq30/openscq30-gui-config.toml` (same file as old GUI).
pub fn config_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    base.join("openscq30").join("openscq30-gui-config.toml")
}

fn read_config(path: &Path) -> GuiConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| toml::from_str(&text).ok())
        .unwrap_or_default()
}

/// Stored preference, if any. Unparseable values are ignored by the caller.
pub fn load_preferred_language(path: &Path) -> Option<String> {
    read_config(path).preferred_language
}

/// Language ids with translations in `openscq30-lib/i18n`.
const SUPPORTED_LANGUAGES: &[&str] = &[
    "en", "arz", "bn", "de", "es", "fr", "he", "id", "it", "ja", "ko", "pl", "pt-BR", "ru", "tr",
    "uk",
];

/// Best-effort `xx` or `xx-YY` from `$LANGUAGE`/`$LANG` (e.g. `pl_PL.UTF-8`).
/// Returns `None` when nothing usable is set.
pub fn detect_system_language() -> Option<String> {
    for key in ["LANGUAGE", "LANG"] {
        if let Ok(raw) = std::env::var(key) {
            let first = raw.split(':').next().unwrap_or("").trim();
            if first.is_empty() || first == "C" || first == "POSIX" {
                continue;
            }
            let tag = first.split('.').next().unwrap_or("").replace('_', "-");
            if !tag.is_empty() {
                let short = tag.split('-').next().unwrap_or("").to_owned();
                // Prefer the full tag when supported, else the bare language.
                if SUPPORTED_LANGUAGES.contains(&tag.as_str()) {
                    return Some(tag);
                }
                if SUPPORTED_LANGUAGES.contains(&short.as_str()) {
                    return Some(short);
                }
                return Some(short);
            }
        }
    }
    None
}

/// Resolve the effective language: stored preference, else system detection.
pub fn effective_language(config_path: &Path) -> Option<String> {
    load_preferred_language(config_path).or_else(detect_system_language)
}

// --- Quick presets ---------------------------------------------------------

/// `[{name, fields: [{settingId, name, displayValue, isEnabled}]}]`.
pub fn quick_presets_json(
    device: &dyn OpenSCQ30Device,
    presets: &[openscq30_lib::storage::QuickPreset],
) -> String {
    let entries: Vec<serde_json::Value> = presets
        .iter()
        .map(|preset| {
            let fields: Vec<serde_json::Value> = preset
                .fields
                .iter()
                .map(|field| {
                    let current = device.setting(&field.setting_id);
                    json!({
                        "settingId": field.setting_id.to_string(),
                        "name": field.setting_id.translate(),
                        "displayValue": localize_value(current.as_ref(), &field.value),
                        "isEnabled": field.is_enabled,
                    })
                })
                .collect();
            json!({ "name": preset.name, "fields": fields })
        })
        .collect();
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
}

// --- Legacy (v1) equalizer profile migration -------------------------------

/// Profile as stored by the v1 `config.toml`.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LegacyEqualizerProfile {
    pub volume_offsets: Vec<i16>,
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct LegacyConfig {
    equalizer_custom_profiles: std::collections::HashMap<String, LegacyEqualizerProfile>,
}

/// All v1 profiles in `config_dir/config.toml`; empty when there is no legacy
/// config. Sorted `[{name, values}]` JSON for QML.
pub fn legacy_profiles_json(config_dir: &Path) -> String {
    let mut entries: Vec<serde_json::Value> = read_legacy_profiles(config_dir)
        .into_iter()
        .map(|(name, profile)| json!({ "name": name, "values": profile.volume_offsets }))
        .collect();
    entries.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    serde_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
}

fn read_legacy_profiles(config_dir: &Path) -> Vec<(String, LegacyEqualizerProfile)> {
    let text = match std::fs::read_to_string(config_dir.join("config.toml")) {
        Ok(text) => text,
        Err(_) => return Vec::new(),
    };
    toml::from_str::<LegacyConfig>(&text)
        .map(|config| config.equalizer_custom_profiles.into_iter().collect())
        .unwrap_or_default()
}

/// Import one v1 profile as a custom equalizer profile without activating it
/// (set + save + revert in one batch sends no packets when nothing changed).
/// Ports `openscq30-gui`'s `migrate_legacy_profile`.
pub async fn migrate_legacy_profile(
    device: &(dyn OpenSCQ30Device + Send + Sync),
    name: &str,
    values: Vec<i16>,
) -> anyhow::Result<()> {
    let old_values = match device.setting(&SettingId::VolumeAdjustments) {
        Some(Setting::Equalizer { value, .. }) => value,
        Some(_) => anyhow::bail!("VolumeAdjustments is not an equalizer"),
        None => anyhow::bail!("device does not have VolumeAdjustments setting"),
    };
    device
        .set_setting_values(vec![
            (SettingId::VolumeAdjustments, values.into()),
            (
                SettingId::CustomEqualizerProfile,
                Value::ModifiableSelectCommand(
                    openscq30_lib::settings::ModifiableSelectCommand::Add(std::borrow::Cow::Owned(
                        name.to_owned(),
                    )),
                ),
            ),
            (SettingId::VolumeAdjustments, old_values.into()),
        ])
        .await?;
    Ok(())
}

/// Raw offsets of one v1 profile, if it exists.
pub fn legacy_profile_values(config_dir: &Path, name: &str) -> Option<Vec<i16>> {
    read_legacy_profiles(config_dir)
        .into_iter()
        .find(|(profile_name, _)| profile_name == name)
        .map(|(_, profile)| profile.volume_offsets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openscq30_lib::settings::{Equalizer, Range, Select};
    use std::borrow::Cow;

    fn select_setting() -> Setting {
        Setting::Select {
            setting: Select {
                options: vec![Cow::Borrowed("a"), Cow::Borrowed("b")],
                localized_options: vec!["A".into(), "B".into()],
            },
            value: Cow::Borrowed("a"),
        }
    }

    #[test]
    fn device_models_json_lists_known_model() {
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&device_models_json()).expect("valid json");
        assert!(!parsed.is_empty());
        assert!(parsed.iter().any(|m| m["id"] == "SoundcoreA3028"));
        // sorted by id
        let ids: Vec<&str> = parsed
            .iter()
            .map(|m| m["id"].as_str().expect("id is a string"))
            .collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn paired_devices_json_roundtrip() {
        let paired = vec![PairedDevice {
            mac_address: parse_mac("01:02:03:04:05:06").unwrap(),
            model: DeviceModel::SoundcoreA3028,
            is_demo: true,
        }];
        let (json_str, count) = paired_devices_json(&paired);
        assert_eq!(count, 1);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["mac"], "01:02:03:04:05:06");
        assert_eq!(parsed[0]["model"], "SoundcoreA3028");
        assert!(parsed[0]["isDemo"].as_bool().unwrap());
    }

    #[test]
    fn rejects_bad_inputs() {
        parse_mac("not-a-mac").unwrap_err();
        parse_model("SoundcoreNope").unwrap_err();
        parse_setting_id("nope").unwrap_err();
        parse_category_id("nope").unwrap_err();
        parse_value("{{{").unwrap_err();
        // Valid camelCase ids parse (strum camelCase).
        assert_eq!(
            parse_setting_id("ambientSoundMode").unwrap(),
            SettingId::AmbientSoundMode
        );
        assert_eq!(
            parse_category_id("equalizer").unwrap(),
            CategoryId::Equalizer
        );
    }

    #[test]
    fn value_json_roundtrips() {
        for (json_str, expected) in [
            (r#"{"type":"bool","value":true}"#, Value::Bool(true)),
            (r#"{"type":"i32","value":-5}"#, Value::I32(-5)),
            (r#"{"type":"f32","value":123.0}"#, Value::F32(123.0)),
            (
                r#"{"type":"string","value":"x"}"#,
                Value::String(Cow::Borrowed("x")),
            ),
            (
                r#"{"type":"optionalString","value":null}"#,
                Value::OptionalString(None),
            ),
            (
                r#"{"type":"i16Vec","value":[1,-2]}"#,
                Value::I16Vec(vec![1, -2]),
            ),
            (
                r#"{"type":"stringVec","value":["a"]}"#,
                Value::StringVec(vec![Cow::Borrowed("a")]),
            ),
        ] {
            assert_eq!(parse_value(json_str).unwrap(), expected, "{json_str}");
        }
        // Select commands used by modifiable/multi-remove controls.
        let cmd = parse_value(
            r#"{"type":"modifiableSelectCommand","value":{"type":"add","name":"new"}}"#,
        )
        .unwrap();
        assert_eq!(
            cmd,
            Value::ModifiableSelectCommand(openscq30_lib::settings::ModifiableSelectCommand::Add(
                Cow::Borrowed("new")
            ))
        );
        let cmd = parse_value(
            r#"{"type":"multiSelectWithRemoveCommand","value":{"type":"remove","name":"old"}}"#,
        )
        .unwrap();
        assert_eq!(
            cmd,
            Value::MultiSelectWithRemoveCommand(
                openscq30_lib::settings::MultiSelectWithRemoveCommand::Remove(Cow::Borrowed("old"))
            )
        );
    }

    #[test]
    fn all_setting_kinds_map() {
        const BANDS: &[u16] = &[100, 400];
        let cases: Vec<(SettingId, Setting, &str)> = vec![
            (
                SettingId::WearingDetection,
                Setting::Toggle { value: true },
                "toggle",
            ),
            (
                SettingId::Volume,
                Setting::I32Range {
                    setting: Range {
                        range: 0..=100,
                        step: 5,
                    },
                    value: 42,
                },
                "range",
            ),
            (SettingId::AmbientSoundMode, select_setting(), "select"),
            (
                SettingId::Ldac,
                Setting::OptionalSelect {
                    setting: Select {
                        options: vec![],
                        localized_options: vec![],
                    },
                    value: None,
                },
                "select",
            ),
            (
                SettingId::CustomEqualizerProfile,
                Setting::ModifiableSelect {
                    setting: Select {
                        options: vec![],
                        localized_options: vec![],
                    },
                    value: None,
                },
                "select",
            ),
            (
                SettingId::NormalModeInCycle,
                Setting::MultiSelect {
                    setting: Select {
                        options: vec![Cow::Borrowed("a")],
                        localized_options: vec!["A".into()],
                    },
                    values: vec![Cow::Borrowed("a")],
                },
                "multi",
            ),
            (
                SettingId::SinglePress,
                Setting::MultiSelectWithRemove {
                    setting: Select {
                        options: vec![],
                        localized_options: vec![],
                    },
                    values: vec![],
                },
                "multi",
            ),
            (
                SettingId::VolumeAdjustments,
                Setting::Equalizer {
                    setting: Equalizer {
                        band_hz: Cow::Borrowed(BANDS),
                        fraction_digits: 1,
                        min: -60,
                        max: 60,
                    },
                    read_only: false,
                    value: vec![0, 10],
                },
                "equalizer",
            ),
            (
                SettingId::PresetEqualizerProfile,
                Setting::PresetEqualizerProfileSelect {
                    equalizer: Equalizer {
                        band_hz: Cow::Borrowed(BANDS),
                        fraction_digits: 1,
                        min: -60,
                        max: 60,
                    },
                    select: Select {
                        options: vec![Cow::Borrowed("a")],
                        localized_options: vec!["A".into()],
                    },
                    presets: vec![vec![0, 0]],
                    value: Some(Cow::Borrowed("a")),
                },
                "presetEqualizer",
            ),
            (
                SettingId::SerialNumber,
                Setting::Information {
                    value: "ABC".into(),
                    translated_value: "ABC".into(),
                },
                "info",
            ),
            (
                SettingId::ImportCustomEqualizerProfiles,
                Setting::ImportString {
                    confirmation_message: Some("sure?".into()),
                },
                "import",
            ),
            (
                SettingId::LightsColor,
                Setting::HueColorPicker { hue: 180.0 },
                "hue",
            ),
            (SettingId::PowerOff, Setting::Action, "action"),
            (
                SettingId::AutoStopTimerDuration,
                Setting::TimeOfDay {
                    minutes_after_midnight: 90,
                },
                "time",
            ),
        ];
        for (id, setting, kind) in &cases {
            let entry = setting_entry(id, setting);
            assert_eq!(entry["kind"], *kind, "wrong kind for {id}");
            assert_eq!(entry["id"], id.to_string());
            assert!(!entry["name"].as_str().unwrap().is_empty());
        }
        // Spot-check range extraction and select flags.
        let range_entry = setting_entry(&SettingId::Volume, &cases[1].1);
        assert_eq!(range_entry["min"], 0);
        assert_eq!(range_entry["max"], 100);
        assert_eq!(range_entry["step"], 5);
        assert_eq!(range_entry["value"], 42);
        let full = settings_json_for_pairs(
            cases
                .into_iter()
                .map(|(id, setting, _kind)| (id, setting))
                .collect(),
        );
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&full).unwrap();
        assert_eq!(parsed.len(), 14);
    }

    #[test]
    fn modes_reported() {
        let ro = setting_entry(
            &SettingId::SerialNumber,
            &Setting::Information {
                value: String::new(),
                translated_value: String::new(),
            },
        );
        assert_eq!(ro["mode"], "readOnly");
        let wo = setting_entry(&SettingId::PowerOff, &Setting::Action);
        assert_eq!(wo["mode"], "writeOnly");
        let rw = setting_entry(&SettingId::Volume, &Setting::Toggle { value: false });
        assert_eq!(rw["mode"], "readWrite");
    }

    /// Headless end-to-end through the exact `lib` calls the bridge makes.
    /// Demo devices need no Bluetooth hardware.
    #[tokio::test]
    async fn demo_pair_connect_and_read_settings() {
        use openscq30_lib::OpenSCQ30Session;

        let session = OpenSCQ30Session::new_with_in_memory_db()
            .await
            .expect("in-memory session");
        let mac = parse_mac("01:02:03:04:05:06").unwrap();
        session
            .pair(PairedDevice {
                mac_address: mac,
                model: DeviceModel::SoundcoreA3028,
                is_demo: true,
            })
            .await
            .expect("pair demo device");

        let (paired_json, paired_count) =
            paired_devices_json(&session.paired_devices().await.expect("paired list"));
        assert_eq!(paired_count, 1);
        assert!(paired_json.contains("01:02:03:04:05:06"));

        let demos = session
            .list_demo_devices(DeviceModel::SoundcoreA3028)
            .await
            .expect("demo list");
        assert!(!demos.is_empty());
        let (scan_json, scan_count) = scan_results_json(&demos);
        let parsed_scan: Vec<serde_json::Value> = serde_json::from_str(&scan_json).unwrap();
        assert_eq!(scan_count as usize, parsed_scan.len());
        // Keys below are the QML contract (`modelData.name/macAddress`).
        assert!(parsed_scan.iter().all(|d| d["name"].is_string()));
        assert!(parsed_scan.iter().all(|d| d["macAddress"].is_string()));
        // Keys below are the QML contract (`modelData.mac/modelName/isDemo`).
        let parsed_paired: Vec<serde_json::Value> = serde_json::from_str(&paired_json).unwrap();
        assert!(parsed_paired.iter().all(|d| d["mac"].is_string()));
        assert!(parsed_paired.iter().all(|d| d["modelName"].is_string()));
        assert!(parsed_paired.iter().all(|d| d["isDemo"].is_boolean()));

        let device = session.connect(mac).await.expect("connect demo");
        let categories = device.categories();
        assert!(!categories.is_empty());
        let cats_json = categories_json(&categories);
        let parsed_cats: Vec<serde_json::Value> = serde_json::from_str(&cats_json).unwrap();
        assert_eq!(parsed_cats.len(), categories.len());

        // Every category's settings must serialize to the QML shape.
        let mut total = 0;
        for category in &categories {
            let json_str = settings_json(&*device, category);
            let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
            for entry in &parsed {
                assert!(entry["id"].is_string());
                assert!(entry["kind"].is_string());
            }
            total += parsed.len();
        }
        assert!(total > 0, "demo device should expose settings");

        session.unpair(mac).await.expect("unpair");
        assert!(session
            .paired_devices()
            .await
            .expect("paired list")
            .is_empty());
    }

    /// Same flow through a file database, mirroring `session::open_session`
    /// (`dirs::config_dir()` honors `XDG_CONFIG_HOME`).
    #[tokio::test]
    async fn demo_flow_with_file_database() {
        use openscq30_lib::OpenSCQ30Session;

        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("XDG_CONFIG_HOME", dir.path());
        let db_path = default_db_path();
        assert!(db_path.starts_with(dir.path()));
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let session = OpenSCQ30Session::new(db_path).await.expect("file session");
        let mac = parse_mac("AA:BB:CC:DD:EE:FF").unwrap();
        session
            .pair(PairedDevice {
                mac_address: mac,
                model: DeviceModel::SoundcoreA3936,
                is_demo: true,
            })
            .await
            .unwrap();
        let device = session.connect(mac).await.expect("connect demo");
        let categories = device.categories();
        assert!(!categories.is_empty());
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&settings_json(&*device, &categories[0])).unwrap();
        assert!(!parsed.is_empty());
        session.unpair(mac).await.unwrap();
        std::env::remove_var("XDG_CONFIG_HOME");
    }

    #[test]
    fn config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("openscq30-gui-config.toml");
        assert_eq!(load_preferred_language(&path), None);
        // Written by hand (or the old GUI): the stored value is honored.
        std::fs::write(&path, "preferred_language = 'pl'\n").unwrap();
        assert_eq!(load_preferred_language(&path).as_deref(), Some("pl"));
        // Corrupt files fall back to defaults instead of panicking.
        std::fs::write(&path, "preferred_language = [1,2,3]").unwrap();
        assert_eq!(load_preferred_language(&path), None);
    }

    #[test]
    fn detects_language_from_env() {
        std::env::set_var("LANGUAGE", "");
        std::env::set_var("LANG", "pl_PL.UTF-8");
        assert_eq!(detect_system_language().as_deref(), Some("pl"));
        std::env::set_var("LANG", "pt_BR.UTF-8");
        assert_eq!(detect_system_language().as_deref(), Some("pt-BR"));
        std::env::set_var("LANG", "C");
        std::env::remove_var("LANGUAGE");
        assert_eq!(detect_system_language(), None);
        std::env::remove_var("LANG");
    }

    #[test]
    fn legacy_profiles_read_from_v1_config() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(legacy_profiles_json(dir.path()), "[]");
        std::fs::write(
            dir.path().join("config.toml"),
            "[equalizer_custom_profiles.bass]\nvolume_offsets = [10, 20]\n",
        )
        .unwrap();
        let parsed: Vec<serde_json::Value> =
            serde_json::from_str(&legacy_profiles_json(dir.path())).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["name"], "bass");
        assert_eq!(parsed[0]["values"], serde_json::json!([10, 20]));
    }

    #[tokio::test]
    async fn quick_preset_crud_headless() {
        use openscq30_lib::OpenSCQ30Session;

        let session = OpenSCQ30Session::new_with_in_memory_db()
            .await
            .expect("session");
        let mac = parse_mac("0A:0B:0C:0D:0E:0F").unwrap();
        session
            .pair(PairedDevice {
                mac_address: mac,
                model: DeviceModel::SoundcoreA3028,
                is_demo: true,
            })
            .await
            .unwrap();
        let device = session.connect(mac).await.unwrap();
        let handler = session.quick_preset_handler();

        handler.save(&*device, "focus".to_owned()).await.unwrap();
        let presets = handler.quick_presets(&*device).await.unwrap();
        assert_eq!(presets.len(), 1);
        assert!(!presets[0].fields.is_empty());

        let json_str = quick_presets_json(&*device, &presets);
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed[0]["name"], "focus");
        assert_eq!(
            parsed[0]["fields"].as_array().unwrap().len(),
            presets[0].fields.len()
        );

        // Enable the first field, then activating must not fail.
        let first = presets[0].fields[0].setting_id;
        handler
            .toggle_field(&*device, "focus".to_owned(), first, true)
            .await
            .unwrap();
        handler.activate(&*device, "focus").await.unwrap();
        handler.delete(&*device, "focus".to_owned()).await.unwrap();
        assert!(handler.quick_presets(&*device).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn legacy_migration_headless() {
        use openscq30_lib::{settings::Setting, OpenSCQ30Session};
        use std::borrow::Cow;

        let session = OpenSCQ30Session::new_with_in_memory_db()
            .await
            .expect("session");
        let mac = parse_mac("11:22:33:44:55:66").unwrap();
        session
            .pair(PairedDevice {
                mac_address: mac,
                model: DeviceModel::SoundcoreA3027,
                is_demo: true,
            })
            .await
            .unwrap();
        let device = session.connect(mac).await.unwrap();

        migrate_legacy_profile(&*device, "Bass", vec![10; 8])
            .await
            .unwrap();

        let Setting::ModifiableSelect { setting, .. } = device
            .setting(&SettingId::CustomEqualizerProfile)
            .expect("custom profile setting")
        else {
            panic!("CustomEqualizerProfile is not a ModifiableSelect");
        };
        assert!(setting.options.contains(&Cow::Borrowed("Bass")));
        // The profile is imported but not activated.
        let Setting::Equalizer { value, .. } = device
            .setting(&SettingId::VolumeAdjustments)
            .expect("eq setting")
        else {
            panic!("VolumeAdjustments is not an Equalizer");
        };
        assert!(value.iter().all(|v| *v == 0));
    }
}

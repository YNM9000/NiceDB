use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{Environment, Result, Store};

pub(crate) const PREFERENCES: &str = "preferences";

pub const DEFAULT_ROW_CAP: u32 = 2_000_000;
const MIN_ROW_CAP: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct EditorPrefs {
    pub font_size: u8,
    pub tab_width: u8,
    pub autocomplete: bool,
    pub uppercase_keywords: bool,
    pub inspections: bool,
    pub continue_on_error: bool,
}

impl Default for EditorPrefs {
    fn default() -> Self {
        Self {
            font_size: 13,
            tab_width: 2,
            autocomplete: true,
            uppercase_keywords: true,
            inspections: true,
            continue_on_error: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum NullDisplay {
    #[default]
    Null,
    Dash,
    Empty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum DateDisplay {
    #[default]
    Raw,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum NumberDisplay {
    #[default]
    Raw,
    Grouped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct ResultsPrefs {
    pub row_cap: u32,
    pub null_display: NullDisplay,
    pub dates: DateDisplay,
    pub numbers: NumberDisplay,
    pub monitor_ms: u32,
}

impl Default for ResultsPrefs {
    fn default() -> Self {
        Self {
            row_cap: DEFAULT_ROW_CAP,
            null_display: NullDisplay::default(),
            dates: DateDisplay::default(),
            numbers: NumberDisplay::default(),
            monitor_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum UnlockMode {
    #[default]
    Timed,
    Session,
    Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ConfirmMode {
    #[default]
    Typed,
    Modal,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct SafetyPrefs {
    pub timeout_local: u32,
    pub timeout_development: u32,
    pub timeout_staging: u32,
    pub timeout_production: u32,
    pub unlock_minutes: u32,
    pub unlock_mode: UnlockMode,
    pub confirm_mode: ConfirmMode,
}

impl Default for SafetyPrefs {
    fn default() -> Self {
        Self {
            timeout_local: 0,
            timeout_development: 0,
            timeout_staging: 300,
            timeout_production: 60,
            unlock_minutes: 10,
            unlock_mode: UnlockMode::default(),
            confirm_mode: ConfirmMode::default(),
        }
    }
}

impl SafetyPrefs {
    pub fn timeout(&self, env: Environment) -> Option<Duration> {
        let secs = match env {
            Environment::Local => self.timeout_local,
            Environment::Development => self.timeout_development,
            Environment::Staging => self.timeout_staging,
            Environment::Production => self.timeout_production,
        };
        (secs > 0).then(|| Duration::from_secs(u64::from(secs)))
    }

    pub fn unlock_for(&self) -> Duration {
        Duration::from_mins(u64::from(self.unlock_minutes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct Preferences {
    pub editor: EditorPrefs,
    pub results: ResultsPrefs,
    pub safety: SafetyPrefs,
    pub keys: BTreeMap<String, String>,
    pub drivers: DriverPrefs,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct DriverPrefs {
    pub db2_clidriver: String,
}

impl Preferences {
    #[must_use]
    pub fn clamped(mut self) -> Self {
        self.editor.font_size = self.editor.font_size.clamp(10, 24);
        self.editor.tab_width = self.editor.tab_width.clamp(1, 8);
        self.results.row_cap = self.results.row_cap.clamp(MIN_ROW_CAP, DEFAULT_ROW_CAP);
        let day = 86_400;
        for t in [
            &mut self.safety.timeout_local,
            &mut self.safety.timeout_development,
            &mut self.safety.timeout_staging,
            &mut self.safety.timeout_production,
        ] {
            *t = (*t).min(day);
        }
        self.safety.unlock_minutes = self.safety.unlock_minutes.clamp(1, 240);
        self.results.monitor_ms = self.results.monitor_ms.min(600_000);
        self.keys.retain(|k, v| !k.is_empty() && v.len() <= 40);
        self
    }
}

impl Store {
    pub fn preferences(&self) -> Result<Preferences> {
        Ok(self
            .get::<Preferences>(PREFERENCES)?
            .unwrap_or_default()
            .clamped())
    }

    pub fn set_preferences(&self, value: &Preferences) -> Result<Preferences> {
        let value = value.clone().clamped();
        self.put(PREFERENCES, &value)?;
        Ok(value)
    }
}

use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

use crate::{Result, Store, prefs::PREFERENCES};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    #[default]
    Registro,
    Tavola,
    Banco,
    Listino,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Palette {
    #[default]
    #[serde(rename = "1")]
    P1,
    #[serde(rename = "2")]
    P2,
    #[serde(rename = "3")]
    P3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Locale {
    #[default]
    It,
    En,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(transparent)]
#[specta(transparent)]
pub struct UiScale(u16);

impl UiScale {
    pub const STEPS: [u16; 5] = [90, 100, 110, 125, 150];

    pub fn new(percent: u16) -> Option<Self> {
        Self::STEPS.contains(&percent).then_some(Self(percent))
    }

    pub fn percent(self) -> u16 {
        self.0
    }

    pub fn zoom(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

impl Default for UiScale {
    fn default() -> Self {
        Self(100)
    }
}

impl<'de> Deserialize<'de> for UiScale {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(raw
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .and_then(Self::new)
            .unwrap_or_default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(transparent)]
#[specta(transparent)]
pub struct Volume(u8);

impl Volume {
    pub fn new(percent: u8) -> Self {
        Self(percent.min(100))
    }

    pub fn percent(self) -> u8 {
        self.0
    }
}

impl Default for Volume {
    fn default() -> Self {
        Self(30)
    }
}

impl<'de> Deserialize<'de> for Volume {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(raw.as_u64().map_or_else(Self::default, |v| {
            Self::new(u8::try_from(v.min(100)).unwrap_or(100))
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SoundDetail {
    #[default]
    All,
    Reduced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SoundTimbre {
    #[default]
    Marimba,
    Ciotola,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent on/off switches persisted as a flat record"
)]
pub struct Appearance {
    pub style: Style,
    pub palette: Palette,
    pub theme: ThemeMode,
    pub locale: Locale,
    pub ui_scale: UiScale,
    pub sounds: bool,
    pub sound_volume: Volume,
    pub sound_detail: SoundDetail,
    pub sound_timbre: SoundTimbre,
    pub sound_gestures: bool,
    pub jingle: bool,
    #[serde(default = "already_set_up")]
    pub setup_done: bool,
}

fn already_set_up() -> bool {
    true
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            style: Style::Listino,
            palette: Palette::P2,
            theme: ThemeMode::Dark,
            locale: Locale::default(),
            ui_scale: UiScale::default(),
            sounds: true,
            sound_volume: Volume::default(),
            sound_detail: SoundDetail::All,
            sound_timbre: SoundTimbre::Marimba,
            sound_gestures: false,
            jingle: true,
            setup_done: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, specta::Type)]
pub struct ErLayout {
    pub tables: Vec<TablePosition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct TablePosition {
    pub namespace: String,
    pub table: String,
    pub x: f64,
    pub y: f64,
}

const APPEARANCE: &str = "appearance";
const SECLAB_VERSIONS: &str = "seclab_versions";

fn layout_key(connection_id: &str, database: &str) -> String {
    format!("er_layout:{connection_id}:{database}")
}

impl Store {
    pub fn appearance(&self) -> Result<Appearance> {
        if let Some(saved) = self.get(APPEARANCE)? {
            return Ok(saved);
        }
        Ok(Appearance {
            setup_done: self.has_been_used()?,
            ..Appearance::default()
        })
    }

    fn has_been_used(&self) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM settings WHERE key = ?1)
                 OR EXISTS (SELECT 1 FROM connections)",
            [PREFERENCES],
            |r| r.get(0),
        )?)
    }

    pub fn set_appearance(&self, value: &Appearance) -> Result<()> {
        self.put(APPEARANCE, value)
    }

    pub fn seclab_versions(&self) -> Result<Option<(String, String)>> {
        self.get(SECLAB_VERSIONS)
    }

    pub fn set_seclab_versions(&self, json: &str, signature: &str) -> Result<()> {
        self.put(SECLAB_VERSIONS, &(json, signature))
    }

    pub fn layout(&self, connection_id: &str, database: &str) -> Result<Option<ErLayout>> {
        self.get(&layout_key(connection_id, database))
    }

    pub fn set_layout(&self, connection_id: &str, database: &str, value: &ErLayout) -> Result<()> {
        self.put(&layout_key(connection_id, database), value)
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> = self
            .conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(raw.map(|s| serde_json::from_str(&s)).transpose()?)
    }

    pub(crate) fn put<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, serde_json::to_string(value)?],
        )?;
        Ok(())
    }
}

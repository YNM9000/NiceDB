use nd_gen::{ColumnGen, DataLocale};
use serde::{Deserialize, Serialize};

use crate::{Result, Store};

pub(crate) const KEY: &str = "gen_profiles";
const MAX_PROFILES: usize = 300;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
pub struct GenSaved {
    pub id: String,
    pub name: String,
    pub connection_id: String,
    pub database: Option<String>,
    pub namespace: String,
    pub table: String,
    pub locale: DataLocale,
    pub seed: u32,
    pub rows: u32,
    pub parents: Option<u32>,
    pub columns: Vec<ColumnGen>,
    #[specta(type = f64)]
    pub saved_ms: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(default)]
pub struct GenProfiles {
    pub profiles: Vec<GenSaved>,
}

impl Store {
    pub fn gen_profiles(&self) -> Result<GenProfiles> {
        Ok(self.get(KEY)?.unwrap_or_default())
    }

    pub fn save_gen_profile(&self, profile: &GenSaved) -> Result<GenProfiles> {
        let mut all = self.gen_profiles()?;
        all.profiles.retain(|p| p.id != profile.id);
        all.profiles.insert(0, profile.clone());
        all.profiles.truncate(MAX_PROFILES);
        self.put(KEY, &all)?;
        Ok(all)
    }

    pub fn delete_gen_profile(&self, id: &str) -> Result<GenProfiles> {
        let mut all = self.gen_profiles()?;
        all.profiles.retain(|p| p.id != id);
        self.put(KEY, &all)?;
        Ok(all)
    }
}

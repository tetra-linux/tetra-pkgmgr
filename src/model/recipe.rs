use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use crate::model::{Checksum, Source};

#[derive(Debug, Deserialize)]
pub struct RecipeSource {
    pub url: String,
    pub hash: String,
}

impl Checksum<blake3::Hash> for RecipeSource {
    fn checksum(&self) -> Result<blake3::Hash> {
        Ok(blake3::Hash::from_hex(&self.hash)?)
    }
}

impl Source for RecipeSource {
    fn url(&self) -> String {
        self.url.clone()
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct Recipe {
    pub name: String,
    pub version: String,
    pub license: String,
    pub maintainer: String,

    #[serde(default)]
    pub sources: Vec<RecipeSource>,
}

impl Recipe {
    pub fn from_path(path: &Path) -> Result<Self> {
        let recipe_str = std::fs::read_to_string(path)?;
        let recipe: Self = serde_yaml::from_str(&recipe_str)?;
        Ok(recipe)
    }
}

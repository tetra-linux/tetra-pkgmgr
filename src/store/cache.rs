use anyhow::{Result, anyhow};
use std::path::{Path, PathBuf};

use crate::TempFile;

#[derive(Debug)]
pub struct Cache {
    pub cache_dir: PathBuf,
}

impl Cache {
    pub fn get_cache_path(&self, hash: blake3::Hash) -> PathBuf {
        let hash_str = hash.to_string();
        let prefix = hash_str[0..2].to_string();

        let mut path = self.cache_dir.join(prefix);
        path.push(hash_str);

        path
    }

    pub fn hash_file(path: &Path) -> Result<blake3::Hash> {
        let mut hasher = blake3::Hasher::new();
        hasher.update_mmap(path)?;
        Ok(hasher.finalize())
    }

    pub fn validate(&self, hash: blake3::Hash) -> Result<bool> {
        let path = self.get_cache_path(hash);

        if !path.is_file() {
            return Ok(false);
        }

        let computed_hash = Self::hash_file(&path)?;
        if hash != computed_hash {
            // Hash did not match, cached file should be removed
            std::fs::remove_file(path)?;
            return Ok(false);
        }

        Ok(true)
    }

    pub fn cache_tmp_file(&self, tmp_file: &TempFile, hash: blake3::Hash) -> Result<()> {
        let prefix = hash.to_string()[0..2].to_string();
        let cache_target_dir = self.cache_dir.join(prefix);

        if !cache_target_dir.is_dir() {
            std::fs::create_dir_all(&cache_target_dir)?;
        }

        let cache_path = self.get_cache_path(hash);
        std::fs::rename(&tmp_file.path, cache_path)?;

        if !self.validate(hash)? {
            return Err(anyhow!("Temporary file checksum does not match {}", hash));
        }

        Ok(())
    }
}

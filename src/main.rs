mod model;

use anyhow::{Result, anyhow};
use curl::easy::Easy;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

use crate::model::PackageId;

#[derive(Debug)]
struct TetraRoot {
    pub root: PathBuf,
}

impl TetraRoot {
    const DEFAULT_TETRA_ROOT: &str = "/var/tetra";

    fn get_tetra_root() -> PathBuf {
        if cfg!(debug_assertions) {
            let root = std::env::var("TETRA_ROOT").unwrap_or(Self::DEFAULT_TETRA_ROOT.to_string());
            return PathBuf::from(root);
        }

        PathBuf::from(Self::DEFAULT_TETRA_ROOT)
    }

    pub fn new() -> Self {
        Self {
            root: Self::get_tetra_root(),
        }
    }

    pub fn repos(&self) -> Result<Vec<Repository>> {
        let mut repos = Vec::new();
        let repo_dir = self.root.join("repo");

        let paths = std::fs::read_dir(repo_dir)?;
        for path in paths {
            let path = path?;
            if path.path().is_dir() {
                let repo = Repository::from_path(&path.path())?;
                repos.push(repo);
            }
        }

        Ok(repos)
    }

    pub fn cache(&self) -> Result<Cache> {
        let cache_dir = self.root.join("cache");

        if !cache_dir.is_dir() {
            std::fs::create_dir_all(&cache_dir)?;
        }

        Ok(Cache { cache_dir })
    }

    pub fn get_temp_dir(&self) -> Result<PathBuf> {
        let tmp_dir = self.root.join("tmp");

        if !tmp_dir.is_dir() {
            std::fs::create_dir_all(&tmp_dir)?;
        }

        Ok(tmp_dir)
    }

    pub fn get_default_arch(&self) -> String {
        let arch_file = self.root.join("arch");
        std::fs::read_to_string(arch_file)
            .unwrap_or("".to_string())
            .trim()
            .to_string()
    }
}

#[derive(Debug)]
struct Downloader<'a, T> {
    source: &'a T,
    tmp_file: TempFile,
    name: &'a str,
}

impl<'a, T> Downloader<'a, T>
where
    T: Source,
{
    pub fn new(root: &TetraRoot, source: &'a T, name: &'a str) -> Result<Self> {
        let tmp_file = TempFile::new(root, source.checksum()?)?;
        Ok(Self {
            source,
            tmp_file,
            name,
        })
    }

    pub fn download(&self) -> Result<()> {
        let pb = ProgressBar::no_length();
        pb.enable_steady_tick(Duration::from_millis(100));
        pb.set_style(
            ProgressStyle::with_template("{wide_msg:!} {percent:>3}% [{bar:25}] {bytes:>11} / {total_bytes:<11} {binary_bytes_per_sec:>13} ETA {eta_precise:8} ")
                .unwrap()
                .progress_chars("=> "),
        );

        pb.set_message(format!("{}/{}", self.name, self.source.checksum()?));

        let mut out_file = File::create(&self.tmp_file.path)?;

        let mut handle = Easy::new();
        handle.url(&self.source.url())?;
        handle.progress(true)?;

        let mut transfer = handle.transfer();

        transfer.progress_function(|total, current, _, _| {
            if total > 0.0 {
                pb.set_length(total as u64);
                pb.set_position(current as u64);
            }

            true
        })?;

        transfer.write_function(|data| {
            out_file.write_all(data).unwrap();
            Ok(data.len())
        })?;

        transfer.perform()?;

        pb.finish();
        Ok(())
    }

    pub fn send_to_cache(&self, cache: &Cache) -> Result<()> {
        cache.cache_tmp_file(&self.tmp_file, self.source.checksum()?)?;

        Ok(())
    }
}

#[derive(Debug)]
struct TempFile {
    pub path: PathBuf,
}

impl TempFile {
    pub fn new(root: &TetraRoot, hash: blake3::Hash) -> Result<Self> {
        let mut path = root.get_temp_dir()?;
        path.push(hash.to_string());

        Ok(Self { path })
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if !self.path.is_file() {
            return;
        }

        if let Err(e) = std::fs::remove_file(&self.path) {
            println!(
                "WARN: Failed to remove temporary file {}, {e}",
                self.path.display()
            );
        }
    }
}

#[derive(Debug)]
struct Cache {
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

#[derive(Debug, Deserialize)]
struct Repository {
    pub name: String,
    pub desc: String,

    #[serde(skip)]
    pub id: String,

    #[serde(skip)]
    pub pkgs_dir: PathBuf,
}

impl Repository {
    pub fn from_path(path: &Path) -> Result<Self> {
        let repo_meta = path.join("repo.yml");

        if repo_meta.is_file() {
            let repo_s = std::fs::read_to_string(repo_meta)?;
            let mut repo: Self = serde_yaml::from_str(&repo_s)?;

            repo.id = path
                .file_name()
                .ok_or(anyhow!("Failed to unwrap repository path name"))?
                .to_string_lossy()
                .to_string();

            repo.pkgs_dir = path.join("pkgs");

            return Ok(repo);
        }

        Err(anyhow!(
            "Failed to load repository {path:#?}, no repository metadata found."
        ))
    }

    pub fn resolve_package_id(
        &self,
        package_id: &PackageId,
        default_arch: &str,
    ) -> Result<PathBuf> {
        let mut recipe_path = PathBuf::from(&self.pkgs_dir);

        recipe_path.push(
            package_id
                .name
                .chars()
                .nth(0)
                .ok_or(anyhow!("Package name was empty"))?
                .to_string(),
        );

        recipe_path.push(&package_id.name);

        if !recipe_path.is_dir() {
            return Err(anyhow!(
                "Package with name {} could not be found.",
                &package_id.name
            ));
        }

        recipe_path.push(&package_id.version);

        if !recipe_path.is_dir() {
            return Err(anyhow!(
                "Package version {} does not exist.",
                &package_id.version
            ));
        }

        for flavour in &package_id.flavours {
            recipe_path.push(flavour);
        }

        if !recipe_path.is_dir() {
            return Err(anyhow!(
                "Specified package flavour combination does not exist."
            ));
        }

        if let Some(arch) = &package_id.arch {
            let mut path_with_arch = recipe_path.join(arch);
            path_with_arch.push("recipe.yml");

            if path_with_arch.is_file() {
                return Ok(path_with_arch);
            } else {
                return Err(anyhow!(
                    "Package architecure was set to {arch}, but package does not supply it."
                ));
            }
        }

        let mut path_with_default_arch = recipe_path.join(default_arch);
        path_with_default_arch.push("recipe.yml");
        if path_with_default_arch.is_file() {
            return Ok(path_with_default_arch);
        }

        let path_with_recipe = recipe_path.join("recipe.yml");
        if path_with_recipe.is_file() {
            return Ok(path_with_recipe);
        }

        Err(anyhow!("Package recipe could not be found."))
    }
}

trait Checksum<T> {
    fn checksum(&self) -> Result<T>;
}

trait Source: Checksum<blake3::Hash> {
    fn url(&self) -> String;
}

#[derive(Debug, Deserialize)]
struct RecipeSource {
    url: String,
    hash: String,
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
struct Recipe {
    name: String,
    version: String,
    license: String,
    maintainer: String,

    #[serde(default)]
    sources: Vec<RecipeSource>,
}

impl Recipe {
    pub fn from_path(path: &Path) -> Result<Self> {
        let recipe_str = std::fs::read_to_string(path)?;
        let recipe: Self = serde_yaml::from_str(&recipe_str)?;
        Ok(recipe)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("tetra <package id>");
        return;
    }

    let package_id = args[1].clone();

    let tetra_root = TetraRoot::new();
    println!("Tetra Root: {:#?}", tetra_root.root);

    let default_arch = tetra_root.get_default_arch();
    println!("Default architecture: {default_arch}");

    let cache = match tetra_root.cache() {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to obtain cache object: {e}");
            return;
        }
    };
    println!("Cache directory: {:#?}", cache.cache_dir);

    let id = PackageId::from_id_str(package_id);

    println!("\nRepo: {}", id.repo);
    println!("Name: {}", id.name);
    println!("Version: {}", id.version);
    println!("Flavours:");

    for flavour in &id.flavours {
        println!("    - {flavour}");
    }

    println!("Arch: {:?}", id.arch);

    let repos = match tetra_root.repos() {
        Ok(r) => r,
        Err(e) => {
            println!("Failed to locate repositories: {e}");
            return;
        }
    };

    for repo in &repos {
        println!("\nId: {}", repo.id);
        println!("Name: {}", repo.name);
        println!("Description: {}", repo.desc);
        println!("Packages Directory: {:#?}", repo.pkgs_dir);
    }

    let repo = match repos.iter().find(|r| r.id == id.repo) {
        Some(r) => r,
        None => {
            println!("\nCannot find repository with ID {}", id.repo);
            return;
        }
    };

    println!("\nSelected repository {}", repo.id);

    let recipe_path = match repo.resolve_package_id(&id, &default_arch) {
        Ok(p) => p,
        Err(e) => {
            println!("\nFailed to resolve package ID: {e}");
            return;
        }
    };

    println!("\nResolved recipe path: {recipe_path:#?}");

    let recipe = match Recipe::from_path(&recipe_path) {
        Ok(r) => r,
        Err(e) => {
            println!("\nFailed to parse package recipe: {e}");
            return;
        }
    };

    println!("\nName: {}", &recipe.name);
    println!("Version: {}", &recipe.version);
    println!("License: {}", &recipe.license);
    println!("Maintainer: {}", &recipe.maintainer);
    println!("Sources:");

    for source in &recipe.sources {
        println!("    - URL: {}", source.url);
        println!("      Hash: {}", source.hash);

        let cache_path = cache.get_cache_path(source.checksum().unwrap());
        println!("      Cache Path: {cache_path:#?}");

        let validated = match cache.validate(source.checksum().unwrap()) {
            Ok(r) => r,
            Err(e) => {
                println!("Cache validation failed: {e}");
                return;
            }
        };

        if !validated {
            let downloader = match Downloader::new(&tetra_root, source, &recipe.name) {
                Ok(d) => d,
                Err(e) => {
                    println!("Error initializing downloader: {e}");
                    return;
                }
            };

            if let Err(e) = downloader.download() {
                println!("Error while downloading: {e}");
                return;
            }

            if let Err(e) = downloader.send_to_cache(&cache) {
                println!("Caching failed: {e}");
            };
        }
    }
}

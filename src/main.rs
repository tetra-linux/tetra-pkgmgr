use anyhow::{Result, anyhow};
use serde::Deserialize;
use std::path::{Path, PathBuf};

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

    pub fn get_default_arch(&self) -> String {
        let arch_file = self.root.join("arch");
        std::fs::read_to_string(arch_file)
            .unwrap_or("".to_string())
            .trim()
            .to_string()
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
            return Err(anyhow!("Package with name {} could not be found.", &package_id.name));
        }
        
        recipe_path.push(&package_id.version);

        if !recipe_path.is_dir() {
            return Err(anyhow!("Package version {} does not exist.", &package_id.version));
        }

        for flavour in &package_id.flavours {
            recipe_path.push(flavour);
        }

        if !recipe_path.is_dir() {
            return Err(anyhow!("Specified package flavour combination does not exist."));
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

#[derive(Debug, Deserialize)]
struct RecipeSource {
    url: String,
    hash: String,
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

#[derive(Debug)]
struct PackageId {
    pub repo: String,
    pub name: String,
    pub version: String,
    pub flavours: Vec<String>,
    pub arch: Option<String>,
}

impl PackageId {
    pub fn from_str(s: String) -> Self {
        let (rest, arch) = if let Some(pos) = s.rfind('#') {
            (s[..pos].to_string(), Some(s[pos + 1..].to_string()))
        } else {
            (s, None)
        };

        let (repo, rest) = if let Some(pos) = rest.find('/') {
            (rest[..pos].to_string(), rest[pos + 1..].to_string())
        } else {
            ("default".to_string(), rest)
        };

        let (name, rest) = if let Some(pos) = rest.find('@') {
            (rest[..pos].to_string(), rest[pos + 1..].to_string())
        } else if let Some(pos) = rest.find(':') {
            // This handles the case where flavours are present, but no version
            (
                rest[..pos].to_string(),
                format!("latest:{}", &rest[pos + 1..]),
            )
        } else {
            (rest, "latest".to_string())
        };

        let mut parts = rest.split(':');
        let version = parts.next().unwrap_or("latest").to_string();
        let flavours = parts.map(|s| s.to_string()).collect::<Vec<_>>();

        Self {
            repo,
            name,
            version,
            flavours,
            arch,
        }
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

    let id = PackageId::from_str(package_id);

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
        println!("    - URL:{}", source.url);
        println!("      Hash: {}", source.hash);
    }
}

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
}

#[derive(Debug)]
struct PackageId {
    pub repo: String,
    pub name: String,
    pub version: String,
    pub flavours: Vec<String>,
    pub arch: String,
}

impl PackageId {
    pub fn from_str(s: String, default_arch: String) -> Self {
        let (rest, arch) = if let Some(pos) = s.rfind('#') {
            (s[..pos].to_string(), s[pos + 1..].to_string())
        } else {
            (s, default_arch)
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

    let id = PackageId::from_str(package_id, "x86_64".to_string());

    println!("\nRepo: {}", id.repo);
    println!("Name: {}", id.name);
    println!("Version: {}", id.version);
    println!("Flavours:");

    for flavour in id.flavours {
        println!("    - {flavour}");
    }

    println!("Arch: {}", id.arch);

    let repos = match tetra_root.repos() {
        Ok(r) => r,
        Err(e) => {
            println!("{e}");
            return;
        }
    };

    for repo in repos {
        println!("\nId: {}", repo.id);
        println!("Name: {}", repo.name);
        println!("Description: {}", repo.desc);
        println!("Packages Directory: {:#?}", repo.pkgs_dir);
    }
}

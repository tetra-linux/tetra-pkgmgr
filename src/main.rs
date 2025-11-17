const DEFAULT_TETRA_ROOT: &str = "/var/tetra";

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

fn get_tetra_root() -> String {
    if cfg!(debug_assertions) {
        return std::env::var("TETRA_ROOT").unwrap_or(DEFAULT_TETRA_ROOT.to_string());
    }

    DEFAULT_TETRA_ROOT.to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("tetra <package id>");
        return;
    }

    let package_id = args[1].clone();

    let tetra_root = get_tetra_root();

    println!("Tetra Root: {tetra_root}");

    let id = PackageId::from_str(package_id, "x86_64".to_string());

    println!("\nRepo: {}", id.repo);
    println!("Name: {}", id.name);
    println!("Version: {}", id.version);
    println!("Flavours:");

    for flavour in id.flavours {
        println!("    - {flavour}");
    }

    println!("Arch: {}", id.arch);
}

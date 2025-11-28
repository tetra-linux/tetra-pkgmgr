#[derive(Debug)]
pub struct PackageId {
    pub repo: String,
    pub name: String,
    pub version: String,
    pub flavours: Vec<String>,
    pub arch: Option<String>,
}

impl PackageId {
    pub fn from_id_str(s: String) -> Self {
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

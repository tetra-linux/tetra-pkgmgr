use anyhow::Result;

pub trait Checksum<T> {
    fn checksum(&self) -> Result<T>;
}

pub trait Source: Checksum<blake3::Hash> {
    fn url(&self) -> String;
}

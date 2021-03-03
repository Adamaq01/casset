use std::{borrow::Cow, collections::HashMap, fs::OpenOptions, io::Read, path::PathBuf};

use crate::{CassetError, Result};

pub trait AssetResolver: Send + Sync {
    fn resolve(&self, path: &str) -> Result<Cow<[u8]>>;

    fn hot_swap_path(&self) -> Option<PathBuf> {
        None
    }
}

pub struct FileSystemResolver {
    base: PathBuf,
}

impl Default for FileSystemResolver {
    fn default() -> Self {
        Self { base: ".".into() }
    }
}

impl FileSystemResolver {
    pub fn new(base: impl Into<PathBuf>) -> Self {
        Self { base: base.into() }
    }
}

impl AssetResolver for FileSystemResolver {
    fn resolve(&self, path: &str) -> Result<Cow<[u8]>> {
        let path = self.base.join(path);
        let mut file = OpenOptions::new().read(true).open(path)?;
        let len = file.metadata().map(|m| m.len())? as usize;
        let mut data = Vec::with_capacity(len);
        if file.read_to_end(&mut data)? != len {
            Err(CassetError::Other("Couldn't read file".into()))
        } else {
            Ok(Cow::Owned(data))
        }
    }

    fn hot_swap_path(&self) -> Option<PathBuf> {
        Some(self.base.clone())
    }
}

// TODO
#[derive(Default)]
pub struct EmbeddedResolver {
    assets: HashMap<String, Vec<u8>>,
}

impl AssetResolver for EmbeddedResolver {
    fn resolve(&self, path: &str) -> Result<Cow<[u8]>> {
        self.assets
            .get(path)
            .map(Vec::as_slice)
            .map(Cow::Borrowed)
            .ok_or(CassetError::Other("Couldn't resolve asset".into()))
    }
}

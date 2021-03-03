use std::{
    borrow::Cow,
    collections::{hash_map::Entry, HashMap, HashSet},
    sync::Mutex,
};

use crate::{
    errors::{CassetError, Result},
    resolvers::AssetResolver,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) struct Dependency(String, HashSet<String>, HashSet<String>);

/// Path -> Path / Dependencies / Dependents
#[derive(Debug)]
pub(crate) struct Dependencies(HashMap<String, Dependency>);

#[allow(dead_code)]
impl Dependencies {
    pub(crate) fn new() -> Self {
        Self(HashMap::new())
    }

    pub(crate) fn register(&mut self, path: impl Into<String>) {
        let path = path.into();
        if let Entry::Vacant(entry) = self.0.entry(path.clone()) {
            entry.insert(Dependency(path, HashSet::new(), HashSet::new()));
        }
    }

    pub(crate) fn is_registered(&self, path: &str) -> bool {
        self.0.contains_key(path)
    }

    pub(crate) fn unregister(&mut self, path: &str) {
        if let Some(entry) = self.0.remove(path) {
            for dependents in &entry.2 {
                self.remove(dependents, path);
            }
        }
    }

    pub(crate) fn add(&mut self, path: &str, dependency: &str) {
        if !self.0.contains_key(path) || !self.0.contains_key(dependency) {
            return;
        }
        let entry = self.0.get_mut(path).unwrap();
        entry.1.insert(dependency.to_string());
        let dep_entry = self.0.get_mut(dependency).unwrap();
        dep_entry.2.insert(path.to_string());
        /*if let (Some(entry), Some(dep_entry)) = (self.0.get_mut(path), self.0.get_mut(dependency)) {
            entry.1.insert(dependency.to_string());
            dep_entry.2.insert(path.to_string());
        }*/
    }

    pub(crate) fn remove(&mut self, path: &str, dependency: &str) {
        if let Some(entry) = self.0.get_mut(path) {
            entry.1.remove(dependency);
            if let Some(dep_entry) = self.0.get_mut(dependency) {
                dep_entry.2.remove(path);
            }
        }
    }

    pub(crate) fn get_dependents(&self, path: &str) -> Option<HashSet<String>> {
        self.0.get(path).map(|dependency| dependency.2.clone())
    }

    pub(crate) fn dependencies(&self) -> Vec<&Dependency> {
        self.0.values().collect()
    }
}

pub(crate) struct DependencyResolver<'a> {
    internal: &'a dyn AssetResolver,
    dependencies: Mutex<HashSet<String>>,
}

impl<'a> DependencyResolver<'a> {
    pub(crate) fn new(internal: &'a dyn AssetResolver) -> Self {
        DependencyResolver {
            internal,
            dependencies: Mutex::new(HashSet::new()),
        }
    }

    pub(crate) fn collect(self) -> Result<HashSet<String>> {
        self.dependencies
            .into_inner()
            .map_err(|err| CassetError::Other(err.to_string()))
    }
}

impl<'a> AssetResolver for DependencyResolver<'a> {
    fn resolve(&self, path: &str) -> Result<Cow<[u8]>> {
        let resolved = self.internal.resolve(path)?;
        self.dependencies
            .lock()
            .map_err(|err| CassetError::Other(err.to_string()))?
            .insert(path.into());
        Ok(resolved)
    }
}

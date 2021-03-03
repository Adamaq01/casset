mod assets;
mod dependencies;
mod errors;
mod events;
mod handle;
mod resolvers;

use dependencies::{Dependencies, DependencyResolver};
use hotwatch::Hotwatch;
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

pub use {assets::*, errors::*, events::*, handle::*, resolvers::*};

#[derive(Debug, Clone)]
struct Metadata(Arc<Mutex<dyn Any + Send>>);
impl Metadata {
    fn new<T: Clone + Send + 'static>(t: T) -> Self {
        Self(Arc::new(Mutex::new(t)) as Arc<Mutex<dyn Any + Send>>)
    }

    fn downcast<T: Clone + Send + 'static>(&self) -> Option<T> {
        (self.0.clone() as Arc<Mutex<dyn Any>>)
            .lock()
            .map(|metadata| metadata.downcast_ref().map(Clone::clone))
            .expect("Metadata holder in poisoned")
    }
}

type LoadFn = fn(&str, &dyn AssetResolver, Option<Metadata>) -> Result<Arc<dyn Any + Send + Sync>>;

struct InternalAsset {
    asset: Arc<dyn Any + Send + Sync>,
    metadata: Option<Metadata>,
    load_function: LoadFn,
}

impl InternalAsset {
    fn new_handle<A: Asset>(&self) -> Result<Handle<A>> {
        Arc::clone(&self.asset)
            .downcast::<A>()
            .map_err(|_| CassetError::Other("Couldn't create asset handle".into()))
            .map(Handle::new)
    }
}

pub struct Casset {
    resolver: Arc<dyn AssetResolver>,
    #[allow(dead_code)]
    hotwatch: Arc<Option<Hotwatch>>,
    assets: Arc<RwLock<HashMap<String, InternalAsset>>>,
    dependencies: Arc<RwLock<Dependencies>>,
    subscribers: Arc<Mutex<Vec<Box<dyn FnMut(AssetEvent) + Send + Sync>>>>,
}

impl Casset {
    pub fn new(resolver: impl AssetResolver + 'static, hot_reload: bool) -> Result<Self> {
        let assets = Arc::new(RwLock::new(HashMap::new()));
        let dependencies = Arc::new(RwLock::new(Dependencies::new()));
        let subscribers = Arc::new(Mutex::new(Vec::new()));
        let resolver = Arc::new(resolver);
        let hotwatch = if hot_reload {
            resolver
                .hot_swap_path()
                .map(|base| -> Result<Hotwatch> {
                    let assets: Arc<RwLock<HashMap<String, InternalAsset>>> = Arc::clone(&assets);
                    let dependencies = Arc::clone(&dependencies);
                    let subscribers = Arc::clone(&subscribers);
                    let resolver = Arc::clone(&resolver);
                    let base = base
                        .canonicalize()
                        .expect("Couldn't canonicalize resources base path");
                    let mut hotwatch = Hotwatch::new_with_custom_delay(Duration::from_millis(200))?;
                    hotwatch.watch(base.clone(), move |event| {
                        if let hotwatch::Event::Write(path) | hotwatch::Event::Create(path) = event
                        {
                            if let Err(err) = || -> Result<()> {
                                let path = path
                                    .strip_prefix(&base)
                                    .map_err(|err| CassetError::ReloadError(err.to_string()))?;
                                let path = path
                                    .as_os_str()
                                    .to_os_string()
                                    .into_string()
                                    .map_err(|err| {
                                        CassetError::ReloadError(err.to_string_lossy().to_string())
                                    })?
                                    .replace("\\", "/");
                                if !dependencies
                                    .read()
                                    .map_err(|err| CassetError::ReloadError(err.to_string()))?
                                    .is_registered(&path)
                                {
                                    return Ok(());
                                }
                                let mut dependencies = dependencies
                                    .write()
                                    .map_err(|err| CassetError::ReloadError(err.to_string()))?;
                                let mut assets = assets
                                    .write()
                                    .map_err(|err| CassetError::ReloadError(err.to_string()))?;
                                let mut reloaded_assets = Vec::new();
                                if let Some(dependents) = dependencies.get_dependents(&path) {
                                    for dependent in dependents {
                                        if let Some(value) = assets.get_mut(&dependent) {
                                            let resolver =
                                                DependencyResolver::new(resolver.as_ref());
                                            let asset = (value.load_function)(
                                                &dependent,
                                                &resolver,
                                                value.metadata.clone(),
                                            )
                                            .map_err(|err| {
                                                CassetError::ReloadError(err.to_string())
                                            })?;
                                            value.asset = asset;
                                            if let Ok(set) = resolver.collect() {
                                                for dependency in set {
                                                    dependencies.register(dependency.clone());
                                                    dependencies.add(&dependent, &dependency);
                                                }
                                            }
                                            reloaded_assets.push(dependent);
                                        }
                                    }
                                }

                                drop(dependencies);
                                drop(assets);

                                if !reloaded_assets.is_empty() {
                                    let event = AssetEvent::Reloaded(reloaded_assets.as_slice());
                                    if let Ok(mut guard) = subscribers.lock() {
                                        Self::emit_event(guard.as_mut(), event);
                                    }
                                }

                                Ok(())
                            }() {
                                log::warn!("{}", err);
                            }
                        }
                    })?;

                    Ok(hotwatch)
                })
                .transpose()?
        } else {
            None
        };

        Ok(Self {
            resolver,
            hotwatch: Arc::new(hotwatch),
            assets,
            dependencies,
            subscribers,
        })
    }

    pub async fn async_load<A: Asset>(
        &self,
        path: &str,
        metadata: Option<A::Metadata>,
    ) -> Result<Handle<A>> {
        self.load(path, metadata)
    }

    /// Loads an asset or returns it from the cache if it has already been loaded
    pub fn load<A: Asset>(&self, path: &str, metadata: Option<A::Metadata>) -> Result<Handle<A>> {
        let assets = self
            .assets
            .read()
            .map_err(|err| CassetError::Other(err.to_string()))?;
        if let Some(asset) = assets.get(path) {
            asset.new_handle()
        } else {
            drop(assets);

            let resolver = DependencyResolver::new(self.resolver.as_ref());
            let cloned_metadata = metadata.clone();
            let asset = Arc::new(A::load(path, &resolver, metadata)?);
            let mut assets = self
                .assets
                .write()
                .map_err(|err| CassetError::Other(err.to_string()))?;
            let mut dependencies = self
                .dependencies
                .write()
                .map_err(|err| CassetError::Other(err.to_string()))?;
            for dependency in resolver.collect()? {
                dependencies.register(dependency.clone());
                dependencies.add(path, &dependency);
            }
            drop(dependencies);
            let asset = InternalAsset {
                asset,
                metadata: cloned_metadata.map(Metadata::new),
                load_function: load_funtion::<A>(),
            };
            let handle = asset.new_handle();
            assets.insert(path.to_string(), asset);
            drop(assets);

            let assets = [path.to_string()];
            let event = AssetEvent::Loaded(&assets);
            if let Ok(mut guard) = self.subscribers.lock() {
                Self::emit_event(guard.as_mut(), event);
            };

            handle
        }
    }

    pub fn get<A: Asset>(&self, path: &str) -> Option<Handle<A>> {
        self.assets
            .read()
            .ok()
            .and_then(|assets| assets.get(path).and_then(|a| a.new_handle().ok()))
    }

    pub fn remove<A: Asset>(&self, path: &str) -> Option<Handle<A>> {
        self.assets
            .write()
            .ok()
            .and_then(|mut assets| assets.remove(path))
            .and_then(|data| {
                let data = data.new_handle().ok();

                let assets = [path.to_string()];
                let event = AssetEvent::Removed(&assets);
                if let Ok(mut guard) = self.subscribers.lock() {
                    Self::emit_event(guard.as_mut(), event);
                }

                data
            })
    }

    pub fn subscribe(&self, subscriber: impl FnMut(AssetEvent) + Send + Sync + 'static) {
        if let Ok(mut guard) = self.subscribers.lock() {
            guard.push(Box::new(subscriber))
        }
    }

    pub(crate) fn emit_event(
        subscribers: &mut Vec<Box<dyn FnMut(AssetEvent) + Send + Sync>>,
        event: AssetEvent,
    ) {
        for subscriber in subscribers.iter_mut() {
            subscriber(event.clone());
        }
    }

    pub fn resolver(&self) -> &dyn AssetResolver {
        self.resolver.as_ref()
    }
}

use std::{any::Any, sync::Arc};

use crate::{resolvers::AssetResolver, LoadFn, Result};

pub trait Asset: Sized + Send + Sync + 'static {
    type Metadata: Clone + Send;

    fn load(
        path: &str,
        resolver: &dyn AssetResolver,
        metadata: Option<Self::Metadata>,
    ) -> Result<Self>;
}

pub(crate) fn load_funtion<A: Asset>() -> LoadFn {
    |path, resolver, metadata| {
        let metadata = metadata.and_then(|metadata| metadata.downcast::<A::Metadata>());
        A::load(path, resolver, metadata)
            .map(Arc::new)
            .map(|arc| arc as Arc<dyn Any + Send + Sync>)
    }
}

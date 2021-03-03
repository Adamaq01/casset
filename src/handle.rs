use std::{ops::Deref, sync::Arc};

use crate::assets::Asset;

#[derive(Debug)]
pub struct Handle<A: Asset> {
    data: Arc<A>,
}

impl<A: Asset> Handle<A> {
    pub(crate) fn new(data: Arc<A>) -> Self {
        Self { data }
    }
}

impl<A: Asset> Deref for Handle<A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        self.data.as_ref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AssetEvent<'a> {
    Loaded(&'a [String]),
    Reloaded(&'a [String]),
    Removed(&'a [String]),
}

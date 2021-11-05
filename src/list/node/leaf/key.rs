mod sealed {
    pub trait Sealed {}
}

use sealed::Sealed;

pub trait StoreKeysOption<T>: Sealed {
    type Key: Clone;
    type Optional: Clone
        + Default
        + From<Option<Self::Key>>
        + Into<Option<Self::Key>>;
    fn as_key(value: &T) -> Self::Key;
}

pub struct StoreKeys;

impl Sealed for StoreKeys {}

impl<T: Clone> StoreKeysOption<T> for StoreKeys {
    type Key = T;
    type Optional = Option<T>;

    fn as_key(value: &T) -> T {
        value.clone()
    }
}

#[derive(Clone, Default)]
pub struct NoKey;

impl From<Option<NoKey>> for NoKey {
    fn from(_: Option<NoKey>) -> Self {
        Self
    }
}

impl Sealed for () {}

impl<T> StoreKeysOption<T> for () {
    type Key = NoKey;
    type Optional = NoKey;

    fn as_key(_value: &T) -> NoKey {
        NoKey
    }
}

use core::convert::Infallible;

pub trait StoreKeysOptionPriv<T> {
    type Key: Clone;
    fn as_key(value: &T) -> Option<Self::Key>;
}

pub struct StoreKeys<const B: bool>;

impl<T: Clone> StoreKeysOptionPriv<T> for StoreKeys<true> {
    type Key = T;

    fn as_key(value: &T) -> Option<T> {
        Some(value.clone())
    }
}

impl<T> StoreKeysOptionPriv<T> for StoreKeys<false> {
    type Key = Infallible;

    fn as_key(_value: &T) -> Option<Infallible> {
        None
    }
}

use StoreKeysOptionPriv as Sealed;
pub trait StoreKeysOption<T>: Sealed<T> {}

impl<T: Clone> StoreKeysOption<T> for StoreKeys<true> {}
impl<T> StoreKeysOption<T> for StoreKeys<false> {}

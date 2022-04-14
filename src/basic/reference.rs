use super::{Align2, BasicLeaf};
use crate::{LeafNext, LeafRef, SetNextParams, StoreKeysOption};
use core::cell::Cell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

#[repr(align(2))]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct RefLeaf<'a, T> {
    pub data: T,
    next: Cell<Option<TaggedPtr<Align2, 1>>>,
    phantom: PhantomData<Cell<&'a Self>>,
}

impl<'a, T> RefLeaf<'a, T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            next: Cell::default(),
            phantom: PhantomData,
        }
    }
}

impl<'a, T> From<T> for RefLeaf<'a, T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T> Deref for RefLeaf<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<'a, T> DerefMut for RefLeaf<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<'a, T> fmt::Debug for RefLeaf<'a, T>
where
    T: BasicLeaf + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("RefLeaf")
            .field("addr", &(self as *const _))
            .field("data", &self.data)
            .field("next", &self.next.get())
            .finish()
    }
}

unsafe impl<'a, T> LeafRef for &RefLeaf<'a, T>
where
    T: BasicLeaf,
    T::StoreKeys: StoreKeysOption<Self>,
{
    const FANOUT: usize = T::FANOUT;
    type Size = T::Size;
    type StoreKeys = T::StoreKeys;
    type Align = Align2;

    fn next(&self) -> Option<LeafNext<Self>> {
        self.next.get().map(|p| match p.get() {
            (ptr, 0) => LeafNext::Leaf(unsafe { ptr.cast().as_ref() }),
            (ptr, _) => LeafNext::Data(ptr.cast()),
        })
    }

    fn set_next(params: SetNextParams<'_, Self>) {
        let (this, next) = params.get();
        this.next.set(next.map(|n| match n {
            LeafNext::Leaf(leaf) => {
                TaggedPtr::new(NonNull::from(leaf).cast(), 0)
            }
            LeafNext::Data(data) => TaggedPtr::new(data.cast(), 1),
        }))
    }

    fn size(&self) -> Self::Size {
        self.data.size()
    }
}

#[cfg(test)]
impl<'a, T> crate::list::debug::LeafDebug for &RefLeaf<'a, T>
where
    T: fmt::Debug + BasicLeaf,
    T::StoreKeys: StoreKeysOption<Self>,
{
    type Id = *const RefLeaf<'a, T>;
    type Data = T;

    fn id(&self) -> Self::Id {
        *self as _
    }

    fn data(&self) -> &T {
        &self.data
    }
}

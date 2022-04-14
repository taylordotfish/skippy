use super::{Align2, BasicLeaf};
use crate::{LeafNext, LeafRef, SetNextParams, StoreKeysOption};
use alloc::rc::Rc;
use core::cell::Cell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

#[repr(align(2))]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct RcLeaf<T> {
    pub data: T,
    next: Cell<Option<TaggedPtr<Align2, 1>>>,
    phantom: PhantomData<Rc<Self>>,
}

impl<T> RcLeaf<T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            next: Cell::default(),
            phantom: PhantomData,
        }
    }
}

impl<T> From<T> for RcLeaf<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<T> Deref for RcLeaf<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T> DerefMut for RcLeaf<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<T> fmt::Debug for RcLeaf<T>
where
    T: BasicLeaf + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("RcLeaf")
            .field("addr", &(self as *const _))
            .field("data", &self.data)
            .field("next", &self.next.get())
            .finish()
    }
}

unsafe impl<T> LeafRef for Rc<RcLeaf<T>>
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
            (ptr, 0) => {
                LeafNext::Leaf(unsafe { Rc::from_raw(ptr.cast().as_ptr()) })
            }
            (ptr, _) => LeafNext::Data(ptr.cast()),
        })
    }

    fn set_next(params: SetNextParams<'_, Self>) {
        let (this, next) = params.get();
        this.next.set(next.map(|n| match n {
            LeafNext::Leaf(leaf) => TaggedPtr::new(
                NonNull::new(Rc::into_raw(leaf) as *mut Self).unwrap().cast(),
                0,
            ),
            LeafNext::Data(data) => TaggedPtr::new(data.cast(), 1),
        }))
    }

    fn size(&self) -> Self::Size {
        self.data.size()
    }
}

#[cfg(test)]
impl<T> crate::list::debug::LeafDebug for Rc<RcLeaf<T>>
where
    T: fmt::Debug + BasicLeaf,
    T::StoreKeys: StoreKeysOption<Self>,
{
    type Id = *const RcLeaf<T>;
    type Data = T;

    fn id(&self) -> Self::Id {
        Rc::as_ptr(self)
    }

    fn data(&self) -> &T {
        &self.data
    }
}

use super::{LeafNext, LeafRef, SetNextParams, StoreKeys, StoreKeysOption};
use core::cell::Cell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{AddAssign, SubAssign};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

mod align {
    #[repr(align(2))]
    pub struct Align2(u16);
}

use align::Align2;

#[repr(align(2))]
pub struct BasicLeaf<'a, T, K = StoreKeys> {
    pub data: T,
    next: Cell<Option<TaggedPtr<Align2, 1>>>,

    #[allow(clippy::type_complexity)]
    phantom: PhantomData<(Cell<&'a Self>, fn() -> K)>,
}

impl<'a, T, K> BasicLeaf<'a, T, K> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            next: Cell::default(),
            phantom: PhantomData,
        }
    }
}

impl<'a, T, K> fmt::Debug for BasicLeaf<'a, T, K>
where
    T: AsSize + fmt::Debug,
    K: for<'b> StoreKeysOption<&'b Self>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BasicLeaf")
            .field("data", &self.data)
            .field("next", &self.next.get())
            .finish()
    }
}

pub trait AsSize {
    const FANOUT: usize = 8;
    type Size: Clone + Default + Ord + AddAssign + SubAssign;

    fn as_size(&self) -> Self::Size {
        Self::Size::default()
    }
}

unsafe impl<'a, T, K> LeafRef for &BasicLeaf<'a, T, K>
where
    T: AsSize,
    K: StoreKeysOption<Self>,
{
    const FANOUT: usize = T::FANOUT;
    type Size = T::Size;
    type StoreKeys = K;
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
        self.data.as_size()
    }
}

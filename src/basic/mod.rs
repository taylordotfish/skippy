use core::ops::{AddAssign, SubAssign};

mod rc;
mod reference;

pub use rc::RcLeaf;
pub use reference::RefLeaf;

pub trait BasicLeaf {
    const FANOUT: usize = 8;
    type Size: Clone + Default + Ord + AddAssign + SubAssign;
    type StoreKeys;

    fn size(&self) -> Self::Size {
        Self::Size::default()
    }
}

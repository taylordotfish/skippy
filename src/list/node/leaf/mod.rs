use super::{AllocItem, Down, InternalNodeRef, Next, NodeRef};
use core::fmt::{self, Debug};
use core::ops::{AddAssign, SubAssign};
use core::ptr::NonNull;

mod key;
use key::StoreKeysOptionPriv;
pub use key::{StoreKeys, StoreKeysOption};

#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct NoSize;

impl Debug for NoSize {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "-")
    }
}

impl AddAssign for NoSize {
    fn add_assign(&mut self, _rhs: Self) {}
}

impl SubAssign for NoSize {
    fn sub_assign(&mut self, _rhs: Self) {}
}

pub type Key<L> = <<L as LeafRef>::StoreKeys as StoreKeysOptionPriv<L>>::Key;

/// # Safety
///
/// * `Self` must not be [`Send`] or [`Sync`].
///
/// * [`Self::next`] must initially return [`None`] until [`Self::set_next`] is
///   called.
///
/// * After [`Self::set_next`] is called (with parameter `params`), future
///   calls to [`Self::next`] must return a value identical to `params.1` until
///   the next call to [`Self::set_next`].
///
/// * Because this type is conceptually a reference, clones produced through
///   [`Clone::clone`] must behave identically to the original object. In
///   particular, if an operation is performed on an object `s` of type `Self`,
///   all clones of `s` (transitively and symmetrically) must behave as if that
///   same operation were performed on them.
pub unsafe trait LeafRef: Clone {
    const FANOUT: usize = 8;
    type Size: Clone + Default + Eq + AddAssign + SubAssign;
    type StoreKeys: StoreKeysOption<Self>;
    type Align;

    fn next(&self) -> Option<LeafNext<Self>>;
    fn set_next(params: SetNextParams<'_, Self>);
    fn size(&self) -> Self::Size {
        Self::Size::default()
    }
}

#[derive(Clone, Debug)]
pub enum LeafNext<L: LeafRef> {
    Leaf(L),
    Data(NonNull<AllocItem<L>>),
}

pub struct SetNextParams<'a, L: LeafRef>(&'a L, Option<LeafNext<L>>);

impl<'a, L: LeafRef> SetNextParams<'a, L> {
    pub fn get(self) -> (&'a L, Option<LeafNext<L>>) {
        (self.0, self.1)
    }
}

impl<L: LeafRef> NodeRef for L {
    type Leaf = L;

    fn next(&self) -> Option<Next<Self>> {
        LeafRef::next(self).map(|next| match next {
            LeafNext::Leaf(node) => Next::Sibling(node),
            LeafNext::Data(data) => {
                // SAFETY: Safe due to the safety requirements of `LeafRef`.
                Next::Parent(unsafe { InternalNodeRef::from_ptr(data) })
            }
        })
    }

    fn set_next(&self, next: Option<Next<Self>>) {
        LeafRef::set_next(SetNextParams(
            self,
            next.map(|next| match next {
                Next::Sibling(node) => LeafNext::Leaf(node),
                Next::Parent(node) => LeafNext::Data(node.as_ptr()),
            }),
        ));
    }

    fn size(&self) -> L::Size {
        LeafRef::size(self)
    }

    fn as_down(&self) -> Down<Self> {
        Down::Leaf(self.clone())
    }

    fn from_down(down: Down<Self>) -> Option<Self> {
        match down {
            Down::Leaf(node) => Some(node),
            _ => None,
        }
    }

    fn key(&self) -> Option<Key<Self>> {
        L::StoreKeys::as_key(self)
    }
}

pub trait LeafExt: LeafRef {
    fn set_next_leaf(&self, next: Option<LeafNext<Self>>) {
        Self::set_next(SetNextParams(self, next));
    }
}

impl<L: LeafRef> LeafExt for L {}

pub trait SizeExt: AddAssign + SubAssign + Sized {
    fn add(mut self, other: Self) -> Self {
        self += other;
        self
    }

    fn sub(mut self, other: Self) -> Self {
        self -= other;
        self
    }
}

impl<T: AddAssign + SubAssign> SizeExt for T {}

use super::leaf::Key;
use super::{Down, LeafRef, Next, NextKind, NodeKind, NodeRef};
use crate::Allocator;
use alloc::alloc::Layout;
use cell_ref::{Cell, CellExt};
use core::cmp::Ordering;
use core::marker::{PhantomData, Unpin};
use core::mem::{ManuallyDrop, MaybeUninit};
use core::ops::Deref;
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

union DownUnion<L: LeafRef> {
    pub leaf: ManuallyDrop<L>,
    pub internal: Option<InternalNodeRef<L>>,
}

impl<L: LeafRef> Default for DownUnion<L> {
    fn default() -> Self {
        Self {
            internal: None,
        }
    }
}

#[repr(transparent)]
pub struct AllocItem<L: LeafRef>(MaybeUninit<InternalNode<L>>);

// SAFETY: We never use the inner value, so we can implement `Send`.
unsafe impl<L: LeafRef> Send for AllocItem<L> {}

// SAFETY: We never use the inner value, so we can implement `Sync`.
unsafe impl<L: LeafRef> Sync for AllocItem<L> {}

impl<L: LeafRef> Unpin for AllocItem<L> {}
impl<L: LeafRef> UnwindSafe for AllocItem<L> {}
impl<L: LeafRef> RefUnwindSafe for AllocItem<L> {}

#[repr(align(4))]
pub struct InternalNode<L: LeafRef> {
    _align: [L::Align; 0],
    next: Cell<InternalNext<L>>,
    down: Cell<DownUnion<L>>,
    pub size: Cell<L::Size>,
    pub len: Cell<usize>,
    pub key: Cell<Option<Key<L>>>,
}

impl<L: LeafRef> Default for InternalNode<L> {
    fn default() -> Self {
        Self {
            _align: [],
            next: Cell::default(),
            down: Cell::default(),
            size: Cell::default(),
            len: Cell::default(),
            key: Cell::default(),
        }
    }
}

impl<L: LeafRef> InternalNode<L> {
    pub fn next(&self) -> Option<Next<InternalNodeRef<L>>> {
        let next = self.next.get();
        next.get().map(|n| match next.kind() {
            NextKind::Sibling => Next::Sibling(n),
            NextKind::Parent => Next::Parent(n),
        })
    }

    pub fn set_next(&self, next: Option<Next<InternalNodeRef<L>>>) {
        self.next.with_mut(|sn| {
            sn.set_kind(match next {
                Some(Next::Parent(_)) => NextKind::Parent,
                _ => NextKind::Sibling,
            });
            sn.set(next.map(|n| match n {
                Next::Sibling(n) => n,
                Next::Parent(n) => n,
            }));
        });
    }

    fn drop_down(&self) {
        let kind = self.next.get().down_kind();
        self.next.with_mut(|n| n.set_down_kind(NodeKind::Internal));
        if kind == NodeKind::Leaf {
            // SAFETY: Safe due to this type's invariants (`down` and
            // `down_kind` are always in sync).
            ManuallyDrop::into_inner(unsafe { self.down.take().leaf });
        }
    }

    pub fn down(&self) -> Option<Down<L>> {
        let next = self.next.take();
        let down = self.down.take();
        let result = if next.down_kind() == NodeKind::Leaf {
            // SAFETY: Safe due to this type's invariants (`down` and
            // `down_kind` are always in sync).
            Some(Down::Leaf(L::clone(unsafe { &down.leaf })))
        } else {
            // SAFETY: Safe due to this type's invariants (`down` and
            // `down_kind` are always in sync).
            unsafe { down.internal }.map(Down::Internal)
        };
        self.down.set(down);
        self.next.set(next);
        result
    }

    pub fn down_as<N: NodeRef<Leaf = L>>(&self) -> Option<N> {
        self.down().and_then(|d| d.into_node())
    }

    pub fn set_down(&self, down: Option<Down<L>>) {
        self.drop_down();
        self.next.with_mut(|n| {
            n.set_down_kind(match down {
                Some(Down::Leaf(_)) => NodeKind::Leaf,
                _ => NodeKind::Internal,
            });
        });
        self.down.set(match down {
            Some(Down::Leaf(node)) => DownUnion {
                leaf: ManuallyDrop::new(node),
            },
            Some(Down::Internal(node)) => DownUnion {
                internal: Some(node),
            },
            None => DownUnion {
                internal: None,
            },
        });
    }

    pub fn size(&self) -> L::Size {
        self.size.get()
    }
}

#[repr(align(4))]
struct Align4(u32);

impl Align4 {
    fn sentinel() -> NonNull<Self> {
        static SENTINEL: Align4 = Self(0);
        NonNull::from(&SENTINEL)
    }
}

struct InternalNext<L: LeafRef>(
    TaggedPtr<Align4, 2>,
    PhantomData<NonNull<InternalNode<L>>>,
);

impl<L: LeafRef> Default for InternalNext<L> {
    fn default() -> Self {
        Self(TaggedPtr::new(Align4::sentinel(), 0), PhantomData)
    }
}

impl<L: LeafRef> Clone for InternalNext<L> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<L: LeafRef> Copy for InternalNext<L> {}

impl<L: LeafRef> InternalNext<L> {
    pub fn get(self) -> Option<InternalNodeRef<L>> {
        let ptr = self.0.ptr();
        if ptr == Align4::sentinel() {
            None
        } else {
            Some(InternalNodeRef(ptr.cast()))
        }
    }

    pub fn set(&mut self, node: Option<InternalNodeRef<L>>) {
        self.0 = TaggedPtr::new(
            node.map_or_else(Align4::sentinel, |n| n.0.cast()),
            self.0.tag(),
        );
    }

    pub fn kind(self) -> NextKind {
        NextKind::from_usize(self.0.tag() & 0b1)
    }

    pub fn set_kind(&mut self, kind: NextKind) {
        let (ptr, tag) = self.0.get();
        self.0 = TaggedPtr::new(ptr, (tag & !0b1) | (kind as usize));
    }

    pub fn down_kind(self) -> NodeKind {
        NodeKind::from_usize((self.0.tag() & 0b10) >> 1)
    }

    pub fn set_down_kind(&mut self, kind: NodeKind) {
        let (ptr, tag) = self.0.get();
        self.0 = TaggedPtr::new(ptr, (tag & !0b10) | ((kind as usize) << 1));
    }
}

pub struct InternalNodeRef<L: LeafRef>(NonNull<InternalNode<L>>);

impl<L: LeafRef> InternalNodeRef<L> {
    pub fn alloc<A: Allocator>(alloc: &A) -> Self {
        let ptr = alloc
            .allocate(Layout::new::<InternalNode<L>>())
            .expect("memory allocation failed")
            .cast::<InternalNode<L>>();
        // SAFETY: `Allocator::allocate` returns valid memory matching the
        // provied layout.
        unsafe {
            ptr.as_ptr().write(InternalNode::default());
        }
        Self(ptr)
    }

    /// # Safety
    ///
    /// * This node must have been allocated by `alloc`.
    /// * Any [`InternalNodeRef`]s that refer to this node must never be
    ///   accessed again. Extra care must be taken because this type is
    ///   [`Copy`].
    pub unsafe fn dealloc<A: Allocator>(self, alloc: &A) {
        // SAFETY: Caller guarantees this node hasn't been destructed already.
        let layout = Layout::for_value(&unsafe { self.0.as_ptr().read() });
        // SAFETY: Checked by caller.
        unsafe {
            alloc.deallocate(self.0.cast(), layout);
        }
    }

    /// # Safety
    ///
    /// `ptr` must have come from a previous call to [`Self::as_ptr`].
    pub unsafe fn from_ptr(ptr: NonNull<AllocItem<L>>) -> Self {
        Self(ptr.cast())
    }

    pub fn as_ptr(self) -> NonNull<AllocItem<L>> {
        NonNull::<InternalNode<L>>::from(&*self).cast()
    }
}

impl<L: LeafRef> NodeRef for InternalNodeRef<L> {
    type Leaf = L;

    fn next(&self) -> Option<Next<Self>> {
        (**self).next()
    }

    fn set_next(&self, next: Option<Next<Self>>) {
        (**self).set_next(next);
    }

    fn size(&self) -> L::Size {
        (**self).size()
    }

    fn as_down(&self) -> Down<L> {
        Down::Internal(*self)
    }

    fn from_down(down: Down<Self::Leaf>) -> Option<Self> {
        match down {
            Down::Internal(node) => Some(node),
            _ => None,
        }
    }

    fn key(&self) -> Option<Key<L>> {
        self.key.get()
    }
}

impl<L: LeafRef> Deref for InternalNodeRef<L> {
    type Target = InternalNode<L>;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Guaranteed by this type's invariants -- this type
        // conceptually represents a static reference.
        unsafe { self.0.as_ref() }
    }
}

impl<L: LeafRef> Clone for InternalNodeRef<L> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl<L: LeafRef> Copy for InternalNodeRef<L> {}

impl<L: LeafRef> PartialOrd for InternalNodeRef<L> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<L: LeafRef> Ord for InternalNodeRef<L> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ptr().cmp(&other.as_ptr())
    }
}

impl<L: LeafRef> PartialEq for InternalNodeRef<L> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl<L: LeafRef> Eq for InternalNodeRef<L> {}

impl<L: LeafRef> Drop for InternalNode<L> {
    fn drop(&mut self) {
        self.drop_down();
    }
}

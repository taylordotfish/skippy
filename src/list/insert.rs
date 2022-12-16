use super::node::{Down, InternalNodeRef, Next, NodeRef};
use super::node::{LeafExt, LeafNext, LeafRef};
use super::split::split;
use super::traverse::get_parent;
use super::{max_node_length, PersistentAlloc};
use crate::allocator::Allocator;
use cell_ref::CellExt;

struct Insertion<N: NodeRef> {
    /// Number of new nodes inserted.
    pub count: usize,
    /// First node in chunk to be inserted.
    pub first: N,
    /// Last node in chunk to be inserted.
    pub last: N,
    /// Change in total list size due to the initial insertion of leaves.
    pub diff: <N::Leaf as LeafRef>::Size,
    /// New root.
    pub root: Option<Down<N::Leaf>>,
}

enum InsertionResult<L: LeafRef> {
    Insertion(Insertion<InternalNodeRef<L>>),
    Done(FinishedInsertion<L>),
}

pub struct FinishedInsertion<L: LeafRef> {
    pub old_root: Down<L>,
    pub new_root: Down<L>,
}

fn handle_insertion<N, A>(
    mut insertion: Insertion<N>,
    alloc: &PersistentAlloc<A>,
) -> InsertionResult<N::Leaf>
where
    N: NodeRef,
    A: Allocator,
{
    let last = insertion.last;
    let first = insertion.first;
    let mut parent = if let Some(parent) = get_parent(last) {
        parent
    } else {
        let root = insertion.root.get_or_insert_with(|| first.as_down());
        if first.next_sibling().is_none() {
            return InsertionResult::Done(FinishedInsertion {
                old_root: root.clone(),
                new_root: first.as_down(),
            });
        }
        // Create new root.
        let root = InternalNodeRef::alloc(alloc);
        root.set_down(Some(first.as_down()));
        root.len.set(1);
        root
    };

    let first_parent = parent;
    let new_len = parent.len.get() + insertion.count;
    let use_fast_insertion =
        new_len <= max_node_length::<N::Leaf>() && insertion.root.is_none();

    let count = if use_fast_insertion {
        let diff = insertion.diff.clone();
        parent.len.set(new_len);
        parent.size.with_mut(|s| *s += diff);
        0
    } else {
        let first: N = parent.down_as().unwrap();
        let mut iter = split(first, new_len);
        let end = parent.next();
        iter.next().unwrap().apply_to(parent);
        let count = iter
            .map(|setup| {
                let node = setup.into_new(alloc);
                parent.set_next(Some(Next::Sibling(node)));
                parent = node;
            })
            .count();
        parent.set_next(end);
        count
    };

    InsertionResult::Insertion(Insertion {
        count,
        first: first_parent,
        last: parent,
        diff: insertion.diff,
        root: insertion.root,
    })
}

pub fn insert_after<L, I, A>(
    mut pos: L,
    items: I,
    alloc: &PersistentAlloc<A>,
) -> FinishedInsertion<L>
where
    L: LeafRef,
    I: Iterator<Item = L>,
    A: Allocator,
{
    let first = pos.clone();
    let end = pos.next();
    let mut size = L::Size::default();
    let count = items
        .map(|item| {
            size += item.size();
            assert!(item.next().is_none(), "item is already in a list");
            pos.set_next_leaf(Some(LeafNext::Leaf(item.clone())));
            pos = item;
        })
        .count();
    pos.set_next_leaf(end);
    let insertion = Insertion {
        count,
        first,
        last: pos,
        diff: size,
        root: None,
    };
    let mut result = handle_insertion(insertion, alloc);
    loop {
        match result {
            InsertionResult::Done(done) => return done,
            InsertionResult::Insertion(insertion) => {
                result = handle_insertion(insertion, alloc);
            }
        }
    }
}

use crate::{Allocator, Global};
use cell_ref::CellExt;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::convert::TryFrom;
use core::iter::{self, FusedIterator};
use core::mem;

#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod debug;
mod destroy;
mod destroy_safety;
mod insert;
mod node;
mod remove;
mod split;
mod traverse;

pub use node::{AllocItem, LeafNext, LeafRef, SetNextParams};
pub use node::{NoSize, StoreKeys, StoreKeysOption};

use destroy::{deconstruct, destroy_node_list};
use destroy_safety::SetUnsafeOnDrop;
use insert::insert_after;
use node::{Down, InternalNodeRef, Key, Next, NodeRef, SizeExt};
use remove::remove;
use traverse::{get_last_sibling, get_parent_info};
use traverse::{get_previous, get_previous_info};

fn min_node_length<L: LeafRef>() -> usize {
    (max_node_length::<L>() + 1) / 2
}

fn max_node_length<L: LeafRef>() -> usize {
    L::FANOUT.max(3)
}

fn roots_match<L: LeafRef>(a: &Down<L>, b: &Down<L>) -> bool {
    type Internal<'a, L> = &'a InternalNodeRef<L>;
    Internal::try_from(a) == Internal::try_from(b)
}

fn propagate_update_diff<N: NodeRef>(
    node: N,
    mut key: Option<Key<N::Leaf>>,
    old_size: <N::Leaf as LeafRef>::Size,
    new_size: <N::Leaf as LeafRef>::Size,
) {
    let any_diff = old_size != new_size;
    let info = get_parent_info(node);
    let mut parent = info.parent;
    let mut position = info.position;

    while let Some(node) = parent {
        key = key.filter(|_| position == 0);
        let mut any_update = false;
        if any_diff {
            node.size.with_mut(|s| {
                *s += new_size.clone();
                *s -= old_size.clone();
            });
            any_update = true;
        }
        if let Some(key) = &key {
            node.key.set(Some(key.clone()));
            any_update = true;
        }
        if !any_update {
            break;
        }
        let info = get_parent_info(node);
        parent = info.parent;
        position = info.position;
    }
}

pub struct SkipList<L, A = Global>
where
    L: LeafRef,
    A: Allocator,
{
    alloc: A,
    root: Option<Down<L>>,
}

impl<L: LeafRef> SkipList<L> {
    pub fn new() -> Self {
        Self::new_in(Global)
    }

    pub fn next(item: L) -> Option<L> {
        let mut node = match NodeRef::next(&item)? {
            Next::Sibling(node) => return Some(node),
            Next::Parent(mut node) => loop {
                node = match node.next()? {
                    Next::Sibling(node) => break node,
                    Next::Parent(node) => node,
                }
            },
        };
        loop {
            node = match node.down().unwrap() {
                Down::Leaf(node) => return Some(node),
                Down::Internal(node) => node,
            };
        }
    }

    pub fn previous(item: L) -> Option<L> {
        let mut node = match get_previous(item)? {
            Next::Sibling(node) => return Some(node),
            Next::Parent(mut node) => loop {
                node = match get_previous(node)? {
                    Next::Sibling(node) => break node,
                    Next::Parent(node) => node,
                }
            },
        };
        loop {
            node = match node.down().unwrap() {
                Down::Leaf(node) => return Some(get_last_sibling(node)),
                Down::Internal(node) => get_last_sibling(node),
            };
        }
    }

    pub fn update<F>(item: L, update: F)
    where
        F: FnOnce(),
    {
        let old_size = item.size();
        update();
        let new_size = item.size();
        propagate_update_diff(item, None, old_size, new_size);
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    pub fn new_in(alloc: A) -> Self {
        Self {
            alloc,
            root: None,
        }
    }

    pub fn size(&self) -> L::Size {
        self.root.as_ref().map_or_else(L::Size::default, |r| r.size())
    }

    pub fn get<S>(&self, pos: &S) -> Option<L>
    where
        S: Ord,
        L::Size: Borrow<S>,
    {
        self.get_with_cmp(|size| pos.cmp(size.borrow()))
    }

    pub fn get_with<S, F>(&self, pos: &S, f: F) -> Option<L>
    where
        S: Ord,
        F: Fn(&L::Size) -> S,
    {
        self.get_with_cmp(|size| pos.cmp(&f(size)))
    }

    /// The argument provided to `cmp` is logically the *right-hand* side of
    /// the comparison.
    fn get_with_cmp<F>(&self, cmp: F) -> Option<L>
    where
        F: Fn(&L::Size) -> Ordering,
    {
        match cmp(&self.size()) {
            Ordering::Less => {}
            Ordering::Equal => {
                return self.last().filter(|n| n.size() == L::Size::default());
            }
            Ordering::Greater => return None,
        }

        let mut node = self.root.clone()?;
        let mut size = L::Size::default();
        loop {
            node = match node {
                Down::Leaf(mut node) => loop {
                    size += node.size();
                    if cmp(&size).is_lt() {
                        return Some(node);
                    }
                    node = node.next_sibling().unwrap();
                },
                Down::Internal(mut node) => loop {
                    let new_size = size.clone().add(node.size());
                    if cmp(&new_size).is_lt() {
                        break node.down().unwrap();
                    }
                    size = new_size;
                    node = node.next_sibling().unwrap();
                },
            }
        }
    }

    pub fn position(&self, item: L) -> L::Size {
        fn add_siblings<N: NodeRef>(
            mut node: N,
            pos: &mut <N::Leaf as LeafRef>::Size,
        ) -> Option<InternalNodeRef<N::Leaf>> {
            loop {
                node = match node.next()? {
                    Next::Parent(parent) => return Some(parent),
                    Next::Sibling(node) => {
                        *pos += node.size();
                        node
                    }
                }
            }
        }

        let mut pos = item.size();
        let mut node = if let Some(parent) = add_siblings(item, &mut pos) {
            parent
        } else {
            return self.size().sub(pos);
        };
        loop {
            node = if let Some(parent) = add_siblings(node, &mut pos) {
                parent
            } else {
                return self.size().sub(pos);
            };
        }
    }

    pub fn insert_after(&mut self, pos: L, item: L) {
        self.insert_after_from(pos, iter::once(item));
    }

    pub fn insert_after_from<I>(&mut self, pos: L, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        let root = self.root.as_ref().expect("`pos` is not from this list");
        let set_unsafe_on_drop = SetUnsafeOnDrop;
        let result = insert_after(pos, items.into_iter(), &self.alloc);
        assert!(
            roots_match(root, &result.old_root),
            "`pos` is not from this list",
        );
        mem::forget(set_unsafe_on_drop);
        self.root = Some(result.new_root);
    }

    pub fn insert_after_opt(&mut self, pos: Option<L>, item: L) {
        self.insert_after_opt_from(pos, iter::once(item));
    }

    pub fn insert_after_opt_from<I>(&mut self, pos: Option<L>, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        if let Some(pos) = pos {
            self.insert_after_from(pos, items);
        } else {
            self.push_front_from(items);
        }
    }

    pub fn insert_before(&mut self, pos: L, item: L) {
        self.insert_before_from(pos, iter::once(item));
    }

    pub fn insert_before_from<I>(&mut self, pos: L, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        self.insert_after_opt_from(SkipList::previous(pos), items);
    }

    pub fn insert_before_opt(&mut self, pos: Option<L>, item: L) {
        self.insert_before_opt_from(pos, iter::once(item));
    }

    pub fn insert_before_opt_from<I>(&mut self, pos: Option<L>, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        if let Some(pos) = pos {
            self.insert_before_from(pos, items);
        } else {
            self.push_back_from(items);
        }
    }

    pub fn push_front(&mut self, item: L) {
        self.push_front_from(iter::once(item));
    }

    pub fn push_front_from<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        let mut iter = items.into_iter();
        let first = match iter.next() {
            Some(item) => item,
            None => return,
        };
        assert!(first.next().is_none(), "item is already in a list");

        let size = first.size();
        let mut parent = None;
        let next = self.root.clone().map(|mut down| {
            loop {
                match down {
                    Down::Leaf(node) => return node,
                    Down::Internal(node) => {
                        node.size.with_mut(|s| *s += size.clone());
                        node.key.set(first.key());
                        down = node.down().unwrap();
                        parent = Some(node);
                    }
                }
            }
        });

        if let Some(parent) = parent {
            parent.set_down(Some(Down::Leaf(first.clone())));
            parent.len.with_mut(|len| *len += 1);
            NodeRef::set_next(&first, Some(Next::Sibling(next.unwrap())));
            self.insert_after_from(first, iter);
        } else if let Some(next) = next {
            debug_assert!(next.next().is_none());
            self.root = Some(Down::Leaf(first.clone()));
            self.insert_after_from(first, iter.chain(iter::once(next)));
        } else {
            debug_assert!(self.root.is_none());
            self.root = Some(Down::Leaf(first.clone()));
            self.insert_after_from(first, iter);
        }
    }

    pub fn push_back(&mut self, item: L) {
        self.push_back_from(iter::once(item));
    }

    pub fn push_back_from<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        self.insert_after_opt_from(self.last(), items);
    }

    pub fn remove(&mut self, item: L) {
        let root = self.root.as_ref().expect("`item` is not from this list");
        let mut result = remove(item);
        assert!(
            roots_match(root, &result.old_root),
            "`item` is not from this list"
        );
        // SAFETY:
        //
        // * Every `InternalNode` in the list was allocated by `self.alloc`.
        // * There are no other `InternalNodeRef`s that refer to these nodes,
        //   since `remove` removed them from the skip list.
        unsafe {
            destroy_node_list(&mut result.removed, &self.alloc);
        }
        self.root = result.new_root;
    }

    pub fn replace(&mut self, old: L, new: L) {
        assert!(new.next().is_none(), "new item is already in a list");
        let old_size = old.size();
        new.set_next(NodeRef::next(&old));
        old.set_next(None);

        let info = get_previous_info(old);
        let (parent, previous) = if let Some(prev) = info.previous {
            (prev.parent, prev.node)
        } else {
            self.root = Some(new.as_down());
            return;
        };

        match previous {
            Next::Parent(parent) => parent.set_down(Some(new.as_down())),
            Next::Sibling(prev) => {
                prev.set_next(Some(Next::Sibling(new.clone())))
            }
        };

        propagate_update_diff(
            parent,
            if info.position == 0 {
                let key = new.key();
                parent.key.set(key.clone());
                key
            } else {
                None
            },
            old_size,
            new.size(),
        );
    }

    pub fn first(&self) -> Option<L> {
        let mut node = self.root.clone()?;
        loop {
            node = match node {
                Down::Leaf(node) => return Some(node),
                Down::Internal(node) => node.down().unwrap(),
            }
        }
    }

    pub fn last(&self) -> Option<L> {
        let mut node = self.root.clone()?;
        loop {
            node = match node {
                Down::Leaf(node) => return Some(get_last_sibling(node)),
                Down::Internal(node) => get_last_sibling(node).down().unwrap(),
            }
        }
    }

    pub fn iter(&self) -> Iter<'_, L, A> {
        Iter {
            iter: BasicIter(self.first()),
            _list: self,
        }
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef<StoreKeys = StoreKeys<true>>,
    A: Allocator,
{
    pub fn insert(&mut self, item: L) -> Result<(), L>
    where
        L: Ord,
    {
        self.insert_after_opt(
            match self.find(&item) {
                Ok(n) => Err(n), // Node already in list
                Err(n) => Ok(n), // Node not in list
            }?,
            item,
        );
        Ok(())
    }

    pub fn find<K>(&self, key: &K) -> Result<L, Option<L>>
    where
        K: Ord,
        L: Borrow<K>,
    {
        self.find_with_cmp(|item| key.cmp(item.borrow()))
    }

    pub fn find_with<K, F>(&self, key: &K, f: F) -> Result<L, Option<L>>
    where
        K: Ord,
        F: Fn(&L) -> K,
    {
        self.find_with_cmp(|item| key.cmp(&f(item)))
    }

    /// The argument provided to `cmp` is logically the *right-hand* side of
    /// the comparison.
    fn find_with_cmp<F>(&self, cmp: F) -> Result<L, Option<L>>
    where
        F: Fn(&L) -> Ordering,
    {
        let mut node = self.root.clone().ok_or(None)?;
        if cmp(&node.key().unwrap()).is_lt() {
            return Err(None);
        }
        loop {
            node = match node {
                Down::Leaf(mut node) => loop {
                    if cmp(&node).is_eq() {
                        return Ok(node);
                    }
                    debug_assert!(cmp(&node).is_gt());
                    node = match node.next_sibling() {
                        None => return Err(Some(node)),
                        Some(n) if cmp(&n).is_lt() => return Err(Some(node)),
                        Some(n) => n,
                    };
                },
                Down::Internal(mut node) => loop {
                    let leaf = node.key().unwrap();
                    if cmp(&leaf).is_eq() {
                        return Ok(leaf);
                    }
                    debug_assert!(cmp(&leaf).is_gt());
                    node = match node.next_sibling() {
                        Some(n) if cmp(&n.key().unwrap()).is_ge() => n,
                        _ => break node.down().unwrap(),
                    };
                },
            }
        }
    }
}

impl<L: LeafRef> Default for SkipList<L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L, A> Drop for SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    fn drop(&mut self) {
        let root = match self.root.take() {
            Some(root) => root,
            None => return,
        };
        let mut nodes = deconstruct(root);
        // SAFETY:
        //
        // * Every `InternalNode` in the list was allocated by `self.alloc`.
        // * This method replaces the root with `None`, so no lingering
        //   `InternalNodeRef`s will exist.
        unsafe {
            destroy_node_list(&mut nodes, &self.alloc);
        }
    }
}

struct BasicIter<L>(Option<L>);

impl<L: LeafRef> Iterator for BasicIter<L> {
    type Item = L;

    fn next(&mut self) -> Option<L> {
        let leaf = self.0.take();
        self.0 = leaf.clone().and_then(|n| SkipList::next(n));
        leaf
    }
}

pub struct Iter<'a, L, A>
where
    L: LeafRef,
    A: Allocator,
{
    iter: BasicIter<L>,
    _list: &'a SkipList<L, A>,
}

impl<'a, L, A> Iterator for Iter<'a, L, A>
where
    L: LeafRef,
    A: Allocator,
{
    type Item = L;

    fn next(&mut self) -> Option<L> {
        self.iter.next()
    }
}

impl<'a, L, A> FusedIterator for Iter<'a, L, A>
where
    L: LeafRef,
    A: Allocator,
{
}

impl<'a, L, A> IntoIterator for &'a SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    type Item = L;
    type IntoIter = Iter<'a, L, A>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct IntoIter<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    iter: BasicIter<L>,
    _list: SkipList<L, A>,
}

impl<L, A> Iterator for IntoIter<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    type Item = L;

    fn next(&mut self) -> Option<L> {
        self.iter.next()
    }
}

impl<L, A> FusedIterator for IntoIter<L, A>
where
    L: LeafRef,
    A: Allocator,
{
}

impl<L, A> IntoIterator for SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    type Item = L;
    type IntoIter = IntoIter<L, A>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            iter: BasicIter(self.first()),
            _list: self,
        }
    }
}

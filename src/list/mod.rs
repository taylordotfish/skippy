/*
 * Copyright (C) [unpublished] taylor.fish <contact@taylor.fish>
 *
 * This file is part of Skippy.
 *
 * Skippy is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published
 * by the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * Skippy is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with Skippy. If not, see <https://www.gnu.org/licenses/>.
 */

use crate::allocator::{Allocator, Global};
use crate::options::{Bool, LeafSize, ListOptions};
use cell_ref::CellExt;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::convert::TryFrom;
use core::iter::{self, FusedIterator};
use core::mem;

#[cfg(skippy_debug)]
pub mod debug;
mod destroy;
mod destroy_safety;
mod insert;
mod node;
mod remove;
mod split;
mod traverse;

use crate::PersistentAlloc;
use destroy::{deconstruct, destroy_node_list};
use destroy_safety::SetUnsafeOnDrop;
use insert::insert_after;
pub use node::{AllocItem, LeafNext, LeafRef, SetNextParams};
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
    old_size: LeafSize<N::Leaf>,
    new_size: LeafSize<N::Leaf>,
) {
    let has_size_diff = old_size != new_size;
    let info = get_parent_info(node);
    let mut parent = info.parent;
    let mut index = info.index;

    while let Some(node) = parent {
        key = key.filter(|_| index == 0);
        let mut updated = false;
        if has_size_diff {
            updated = true;
            node.size.with_mut(|s| {
                *s += new_size.clone();
                *s -= old_size.clone();
            });
        }
        if let Some(key) = &key {
            updated = true;
            node.key.set(Some(key.clone()));
        }
        if !updated {
            break;
        }
        let info = get_parent_info(node);
        parent = info.parent;
        index = info.index;
    }
}

pub struct SkipList<L, A = Global>
where
    L: LeafRef,
    A: Allocator,
{
    alloc: PersistentAlloc<A>,
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

    /// The returned iterator will yield `item` as its first element.
    pub fn iter_at(item: L) -> Iter<L> {
        Iter(Some(item))
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    pub fn new_in(alloc: A) -> Self
    where
        A: 'static,
    {
        Self {
            alloc: PersistentAlloc::new(alloc),
            root: None,
        }
    }

    pub fn size(&self) -> LeafSize<L> {
        self.root.as_ref().map_or_else(Default::default, |r| r.size())
    }

    pub fn get<S>(&self, index: &S) -> Option<L>
    where
        S: Ord + ?Sized,
        LeafSize<L>: Borrow<S>,
    {
        self.get_with_cmp(|size| size.borrow().cmp(index))
    }

    /// For this method to yield correct results, `S` and [`LeafSize<L>`] must
    /// form a total order ([`PartialOrd::partial_cmp`] should always return
    /// [`Some`]).
    ///
    /// # Panics
    ///
    /// This method may panic if `S` and [`LeafSize<L>`] do not form a total
    /// order.
    pub fn get_with<S>(&self, index: &S) -> Option<L>
    where
        S: ?Sized,
        LeafSize<L>: PartialOrd<S>,
    {
        self.get_with_cmp(|size| {
            size.partial_cmp(index).unwrap_or_else(
                #[cold]
                || panic!("`partial_cmp` returned `None`"),
            )
        })
    }

    /// Gets the item at the given index using the given comparison function.
    ///
    /// `cmp` checks whether its argument is less than, equal to, or greater
    /// than the desired item. Thus, the argument provided to `cmp` is
    /// logically the *left-hand* side of the comparison.
    pub fn get_with_cmp<F>(&self, cmp: F) -> Option<L>
    where
        F: Fn(&LeafSize<L>) -> Ordering,
    {
        match cmp(&self.size()) {
            Ordering::Less => return None,
            Ordering::Equal => {
                return self.last().filter(|n| n.size() == Default::default());
            }
            Ordering::Greater => {}
        }

        let mut node = self.root.clone()?;
        let mut size = LeafSize::<L>::default();
        loop {
            node = match node {
                Down::Leaf(mut node) => loop {
                    size += node.size();
                    if cmp(&size).is_gt() {
                        return Some(node);
                    }
                    node = node.next_sibling().unwrap();
                },
                Down::Internal(mut node) => loop {
                    let new_size = size.clone().add(node.size());
                    if cmp(&new_size).is_gt() {
                        break node.down().unwrap();
                    }
                    size = new_size;
                    node = node.next_sibling().unwrap();
                },
            }
        }
    }

    pub fn index(&self, item: L) -> LeafSize<L> {
        fn add_siblings<N: NodeRef>(
            mut node: N,
            index: &mut LeafSize<N::Leaf>,
        ) -> Option<InternalNodeRef<N::Leaf>> {
            loop {
                node = match node.next()? {
                    Next::Parent(parent) => return Some(parent),
                    Next::Sibling(node) => {
                        *index += node.size();
                        node
                    }
                }
            }
        }

        let mut index = item.size();
        let mut node = if let Some(parent) = add_siblings(item, &mut index) {
            parent
        } else {
            return self.size().sub(index);
        };
        loop {
            node = if let Some(parent) = add_siblings(node, &mut index) {
                parent
            } else {
                return self.size().sub(index);
            };
        }
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator + 'static,
{
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
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    pub fn replace(&mut self, old: L, new: L) {
        assert!(new.next().is_none(), "new item is already in a list");
        let old_size = old.size();
        new.set_next(NodeRef::next(&old));
        old.set_next(None);

        let info = get_previous_info(new.clone());
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
            if info.index == 0 {
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

    pub fn iter(&self) -> Iter<L> {
        Iter(self.first())
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
    L::Options: ListOptions<L, StoreKeys = Bool<true>>,
{
    pub fn insert(&mut self, item: L) -> Result<(), L>
    where
        L: Ord,
        A: 'static,
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
        K: Ord + ?Sized,
        L: Borrow<K>,
    {
        self.find_with_cmp(|item| item.borrow().cmp(key))
    }

    /// For this method to yield correct results, `K` and `L` must form a
    /// total order ([`PartialOrd::partial_cmp`] should always return
    /// [`Some`]).
    ///
    /// # Panics
    ///
    /// This method may panic if `K` and `L` do not form a total order.
    pub fn find_with<K>(&self, key: &K) -> Result<L, Option<L>>
    where
        K: ?Sized,
        L: PartialOrd<K>,
    {
        self.find_with_cmp(|item| {
            item.partial_cmp(key).unwrap_or_else(
                #[cold]
                || panic!("`partial_cmp` returned `None`"),
            )
        })
    }

    /// Finds an item using the given comparison function.
    ///
    /// `cmp` checks whether its argument is less than, equal to, or greater
    /// than the desired item. Thus, the argument provided to `cmp` is
    /// logically the *left-hand* side of the comparison.
    pub fn find_with_cmp<F>(&self, cmp: F) -> Result<L, Option<L>>
    where
        F: Fn(&L) -> Ordering,
    {
        let mut node = self.root.clone().ok_or(None)?;
        if cmp(&node.key().unwrap()).is_gt() {
            return Err(None);
        }
        loop {
            node = match node {
                Down::Leaf(mut node) => loop {
                    if cmp(&node).is_eq() {
                        return Ok(node);
                    }
                    debug_assert!(cmp(&node).is_lt());
                    node = match node.next_sibling() {
                        None => return Err(Some(node)),
                        Some(n) if cmp(&n).is_gt() => return Err(Some(node)),
                        Some(n) => n,
                    };
                },
                Down::Internal(mut node) => loop {
                    let leaf = node.key().unwrap();
                    if cmp(&leaf).is_eq() {
                        return Ok(leaf);
                    }
                    debug_assert!(cmp(&leaf).is_lt());
                    node = match node.next_sibling() {
                        Some(n) if cmp(&n.key().unwrap()).is_le() => n,
                        _ => break node.down().unwrap(),
                    };
                },
            }
        }
    }
}

impl<L, A> Default for SkipList<L, A>
where
    L: LeafRef,
    A: Allocator + Default + 'static,
{
    fn default() -> Self {
        Self::new_in(A::default())
    }
}

impl<L, A> Drop for SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    fn drop(&mut self) {
        let mut nodes = deconstruct(match self.root.take() {
            Some(root) => root,
            None => return,
        });

        // SAFETY:
        //
        // * Every `InternalNode` in the list was allocated by `self.alloc`.
        // * There are no other `InternalNodeRef`s that refer to these nodes,
        //   since we replaced `self.root` with `None`.
        unsafe {
            destroy_node_list(&mut nodes, &self.alloc);
        }

        // SAFETY:
        //
        // * We just destroyed all `InternalNode`s, so all memory allocated by
        //   `self.alloc` has been deallocated.
        // * We never use `self.alloc` after calling `drop` here.
        unsafe {
            self.alloc.drop();
        }
    }
}

pub struct Iter<L>(Option<L>);

impl<L: LeafRef> Iterator for Iter<L> {
    type Item = L;

    fn next(&mut self) -> Option<L> {
        let leaf = self.0.take();
        self.0 = leaf.clone().and_then(SkipList::next);
        leaf
    }
}

impl<L: LeafRef> FusedIterator for Iter<L> {}

impl<L, A> IntoIterator for &SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    type Item = L;
    type IntoIter = Iter<L>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct IntoIter<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    iter: Iter<L>,
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
            iter: Iter(self.first()),
            _list: self,
        }
    }
}

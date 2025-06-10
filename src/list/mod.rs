/*
 * Copyright (C) 2025 taylor.fish <contact@taylor.fish>
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
use crate::options::{LeafSize, ListOptions};
use cell_ref::CellExt;
use core::borrow::Borrow;
use core::cmp::Ordering;
use core::convert::TryFrom;
use core::iter::once;
use core::marker::PhantomData;
use core::mem;
use integral_constant::{Bool, Constant};

#[cfg(skippy_debug)]
pub mod debug;
mod destroy;
mod destroy_safety;
mod insert;
pub mod iter;
mod node;
mod remove;
mod split;
mod traverse;

use crate::PersistentAlloc;
use destroy::{deconstruct, destroy_node_list};
use destroy_safety::SetUnsafeOnDrop;
use insert::insert_after;
use iter::Iter;
pub use node::{AllocItem, LeafNext, LeafRef, This};
use node::{Down, InternalNodeRef, Key, Next, NodeRef, SizeExt};
use remove::remove;
use traverse::{get_last_sibling, get_parent_info};
use traverse::{get_previous, get_previous_info};

fn min_node_length<L: LeafRef>() -> usize {
    (max_node_length::<L>() + 1) / 2
}

fn max_node_length<L: LeafRef>() -> usize {
    <L::Options as ListOptions>::Fanout::VALUE.max(3)
}

fn roots_match<L: LeafRef>(a: &Down<L>, b: &Down<L>) -> bool {
    type Internal<'a, L> = &'a InternalNodeRef<L>;
    Internal::try_from(a) == Internal::try_from(b)
}

/// Propagate a change in the size of an item (or the item itself, which could
/// change [`Key`]s) throughout the list.
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

/// A flexible intrusive skip list with worst-case non-amortized O(log *n*)
/// operations.
///
/// # Concurrency
///
/// This type is neither [`Send`] nor [`Sync`], and `L` should also not
/// implement either of those traits. However, if you're using a [`SkipList`]
/// and items of type `L` internally within another type, and you can guarantee
/// that, under certain conditions, no other thread could possibly use that
/// particular skip list or its items, it may be safe to send that skip list
/// and all of its items to another thread (but this must be internal---users
/// cannot have direct access to the skip list or items).
///
/// Similarly, if you can guarantee that, under certain conditions, no thread
/// could possibly call any methods of [`SkipList`] (with that particular skip
/// list or involving any of its items, even when called on a different skip
/// list) *except* for `&self` methods (non-methods are okay), it may be safe
/// to use that skip list and those items immutably from multiple threads
/// concurrently (which could involve sending <code>[&][r][SkipList]</code> and
/// `L` across threads). Again, this must be internal---users cannot have
/// direct access to the skip list or items.
///
/// Additionally, no methods of the skip list to be used concurrently should
/// ever have been called with leaf items (of type `L`) that already belonged
/// to another list. Panics may occur when this is done, but whether or not a
/// panic occurs, this can result in the skip list containing items from
/// another list.
///
/// # Mathematical variables
///
/// For the purposes of specifying the time complexity of various operations,
/// *n* refers to the number of items in the list.
///
/// [r]: reference
pub struct SkipList<L, A = Global>
where
    L: LeafRef,
    A: Allocator,
{
    alloc: PersistentAlloc<A>,
    root: Option<Down<L>>,
    /// Ensures that [`Self`] isn't [`Send`] or [`Sync`].
    phantom: PhantomData<*mut ()>,
}

impl<L: LeafRef> SkipList<L> {
    /// Creates a new skip list.
    pub fn new() -> Self {
        Self::new_in(Global)
    }

    /// Gets the item directly after `item`.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*), but a traversal through the entire list by
    /// repeatedly calling this method is only Θ(*n*).
    ///
    /// # Note
    ///
    /// This function is defined only on `SkipList<L>` rather than all
    /// `SkipList<L, A>`, but it can be used with items from any skip list,
    /// including those with custom allocators. Defining the function this way
    /// ensures that `SkipList::next(some_item)` isn't ambiguous. (This applies
    /// to all non-method functions of `SkipList<L>`.)
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

    /// Gets the item directly before `item`.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*), but a traversal through the entire list by
    /// repeatedly calling this method is only Θ(*n*). In practice, this
    /// method is slower than [`Self::next`] by a constant factor.
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

    /// Creates an iterator that starts at `item`.
    ///
    /// The returned iterator will yield `item` as its first element. See also
    /// [`Self::iter`].
    ///
    /// # Time complexity
    ///
    /// Iteration over the entire list is Θ(*n*).
    pub fn iter_at(item: L) -> Iter<L> {
        Iter(Some(item))
    }

    fn subtree_first(first_child: Down<L>) -> L {
        let mut node = first_child;
        loop {
            node = match node {
                Down::Leaf(node) => return node,
                Down::Internal(node) => node.down().unwrap(),
            }
        }
    }

    fn subtree_last(first_child: Down<L>) -> L {
        let mut node = first_child;
        loop {
            node = match node {
                Down::Leaf(node) => return get_last_sibling(node),
                Down::Internal(node) => get_last_sibling(node).down().unwrap(),
            }
        }
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    /// Creates a new skip list with the given allocator.
    pub fn new_in(alloc: A) -> Self
    where
        A: 'static,
    {
        Self {
            alloc: PersistentAlloc::new(alloc),
            root: None,
            phantom: PhantomData,
        }
    }

    /// Gets the total size of the list.
    ///
    /// This is the sum of [`L::size`](LeafRef::size) for every item in the
    /// list.
    ///
    /// # Time complexity
    ///
    /// Constant.
    pub fn size(&self) -> LeafSize<L> {
        self.root.as_ref().map_or_else(Default::default, |r| r.size())
    }

    /// Gets an item by index.
    ///
    /// Note that if there are items with a size of 0, this method will return
    /// the first non–zero-sized item at `index`, or the last item in the list
    /// if `index` is [`self.size()`](Self::size) and the list ends with a
    /// zero-sized item.
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
    pub fn get<S>(&self, index: &S) -> Option<L>
    where
        S: Ord + ?Sized,
        LeafSize<L>: Borrow<S>,
    {
        self.get_with_cmp(|size| size.borrow().cmp(index))
    }

    /// Gets an item by index with a size type that [`LeafSize<L>`] can't be
    /// borrowed as.
    ///
    /// For this method to yield correct results, `S` and [`LeafSize<L>`] must
    /// form a total order ([`PartialOrd::partial_cmp`] should always return
    /// [`Some`]).
    ///
    /// Note that if there are items with a size of 0, this method will return
    /// the first non–zero-sized item at `index`, or the last item in the list
    /// if `index` is [`self.size()`](Self::size) and the list ends with a
    /// zero-sized item.
    ///
    /// # Panics
    ///
    /// This method may panic if `S` and [`LeafSize<L>`] do not form a total
    /// order.
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
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

    /// Gets an item by index using the given comparison function.
    ///
    /// `cmp` checks whether its argument is less than, equal to, or greater
    /// than the desired index. Thus, the argument provided to `cmp` is
    /// logically the *left-hand* side of the comparison.
    ///
    /// Note that if there are items with a size of 0, this method will return
    /// the first non–zero-sized item at the desired index, or the last item in
    /// the list if the desired index is [`self.size()`](Self::size) and the
    /// list ends with a zero-sized item.
    ///
    /// # Panics
    ///
    /// This method may panic if `cmp` returns results inconsistent with the
    /// total order on [`LeafSize<L>`].
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
    pub fn get_with_cmp<F>(&self, cmp: F) -> Option<L>
    where
        F: Fn(&LeafSize<L>) -> Ordering,
    {
        SkipList::subtree_get(cmp, self.root.clone()?, Default::default())
    }
}

impl<L: LeafRef> SkipList<L> {
    /// Gets the index of `item`.
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
    pub fn index(item: L) -> LeafSize<L> {
        fn add_siblings<N: NodeRef>(
            mut node: N,
            index: &mut LeafSize<N::Leaf>,
        ) -> Result<InternalNodeRef<N::Leaf>, N> {
            loop {
                node = match node.next().ok_or(node)? {
                    Next::Parent(parent) => return Ok(parent),
                    Next::Sibling(node) => {
                        *index += node.size();
                        node
                    }
                }
            }
        }

        let mut index = item.size();
        let mut node = match add_siblings(item, &mut index) {
            Ok(parent) => parent,
            Err(_) => {
                return Default::default();
            }
        };
        loop {
            node = match add_siblings(node, &mut index) {
                Ok(parent) => parent,
                Err(node) => {
                    return node.size().sub(index);
                }
            };
        }
    }

    fn subtree_get<F>(
        cmp: F,
        first_child: Down<L>,
        offset: LeafSize<L>,
    ) -> Option<L>
    where
        F: Fn(&LeafSize<L>) -> Ordering,
    {
        let mut node = first_child;
        let mut size = offset;
        loop {
            node = match node {
                Down::Leaf(mut node) => loop {
                    let new_size = size.clone().add(node.size());
                    let ord = cmp(&new_size);
                    if ord.is_le() {
                        if let Some(next) = node.next_sibling() {
                            node = next;
                            size = new_size;
                            continue;
                        }
                        if !(ord.is_eq() && size == new_size) {
                            return None;
                        }
                        // Item is the last element of the list, has a size of
                        // zero, and is at the right index.
                    }
                    return Some(node);
                },
                Down::Internal(mut node) => loop {
                    let new_size = size.clone().add(node.size());
                    let ord = cmp(&new_size);
                    if ord.is_le() {
                        if let Some(next) = node.next_sibling() {
                            node = next;
                            size = new_size;
                            continue;
                        }
                        if !ord.is_eq() {
                            return None;
                        }
                    }
                    break node.down().unwrap();
                },
            }
        }
    }

    /// Gets an item by index, relative to the index of another item.
    ///
    /// This method returns the item whose index is `offset` greater than the
    /// index of `start`. As with [`Self::get`], this method will not return
    /// items with a size of 0 unless the desired index (`offset` plus
    /// <code>[Self::index]\(start)</code>) is equal to the [size] of the list,
    /// in which case the last item of the list will be returned if it has a
    /// size of zero.
    ///
    /// [index]: Self::index
    /// [size]: Self::size
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn get_after<S>(start: L, offset: &S) -> Option<L>
    where
        S: Ord + ?Sized,
        LeafSize<L>: Borrow<S>,
    {
        Self::get_after_with_cmp(start, |size| size.borrow().cmp(offset))
    }

    /// Gets an item by index, relative to the index of another item, using
    /// a size type that [`LeafSize<L>`] can't be borrowed as.
    ///
    /// This method is to [`Self::get_after`] what [`Self::get_with`] is to
    /// [`Self::get`].
    ///
    /// For this method to yield correct results, `S` and [`LeafSize<L>`] must
    /// form a total order ([`PartialOrd::partial_cmp`] should always return
    /// [`Some`]).
    ///
    /// This method returns the item whose index is `offset` greater than the
    /// index of `start`. As with [`Self::get`], this method will not return
    /// items with a size of 0 unless the desired index (`offset` plus
    /// <code>[Self::index]\(start)</code>) is equal to the [size] of the list,
    /// in which case the last item of the list will be returned if it has a
    /// size of zero.
    ///
    /// [index]: Self::index
    /// [size]: Self::size
    ///
    /// # Panics
    ///
    /// This method may panic if `S` and [`LeafSize<L>`] do not form a total
    /// order.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn get_after_with<S>(start: L, offset: &S) -> Option<L>
    where
        S: ?Sized,
        LeafSize<L>: PartialOrd<S>,
    {
        Self::get_after_with_cmp(start, |size| {
            size.partial_cmp(offset).unwrap_or_else(
                #[cold]
                || panic!("`partial_cmp` returned `None`"),
            )
        })
    }

    /// Gets an item by index, relative to the index of another item, using the
    /// given comparison function.
    ///
    /// This method is to [`Self::get_after`] what [`Self::get_with_cmp`] is to
    /// [`Self::get`].
    ///
    /// `cmp` checks whether its argument is less than, equal to, or greater
    /// than the desired index. Thus, the argument provided to `cmp` is
    /// logically the *left-hand* side of the comparison.
    ///
    /// Given `offset` as the [`LeafSize<L>`] for which `cmp(offset)` returns
    /// [`Ordering::Equal`], this method returns the item whose index is
    /// `offset` greater than the index of `start`. As with [`Self::get`], this
    /// method will not return items with a size of 0 unless the desired index
    /// (`offset` plus <code>[Self::index]\(start)</code>) is equal to the
    /// [size] of the list, in which case the last item of the list will be
    /// returned if it has a size of zero.
    ///
    /// [index]: Self::index
    /// [size]: Self::size
    ///
    /// # Panics
    ///
    /// This method may panic if `cmp` returns results inconsistent with the
    /// total order on [`LeafSize<L>`].
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn get_after_with_cmp<F>(start: L, cmp: F) -> Option<L>
    where
        F: Fn(&LeafSize<L>) -> Ordering,
    {
        let mut leaf = start;
        let mut size = LeafSize::<L>::default();
        let mut ord;
        let mut internal = loop {
            let old_size = size.clone();
            size += leaf.size();
            ord = cmp(&size);
            if ord.is_le() {
                match NodeRef::next(&leaf) {
                    Some(Next::Sibling(next)) => {
                        leaf = next;
                        continue;
                    }
                    Some(Next::Parent(node)) => break node,
                    // If this match arm is taken: the item is the last element
                    // of the list, has a size of zero, and is at the right
                    // index.
                    None if ord.is_eq() && old_size == size => {}
                    None => return None,
                }
            }
            return Some(leaf);
        };

        let mut leaf_is_last = true;
        loop {
            match internal.next() {
                Some(Next::Sibling(next)) => {
                    internal = next;
                    leaf_is_last = false;
                }
                Some(Next::Parent(node)) => {
                    internal = node;
                    continue;
                }
                None if ord.is_eq() => {
                    let last = if leaf_is_last {
                        leaf
                    } else {
                        Self::subtree_last(internal.as_down())
                    };
                    return if last.size() == Default::default() {
                        Some(last)
                    } else {
                        None
                    };
                }
                None => return None,
            }
            let new_size = size.clone().add(internal.size());
            ord = cmp(&new_size);
            if ord.is_gt() {
                return Self::subtree_get(cmp, internal.down().unwrap(), size);
            }
            size = new_size;
        }
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    /// Inserts `item` directly after `pos`.
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if `item` is
    /// already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn insert_after(&mut self, pos: L, item: L) {
        self.insert_after_from(pos, once(item));
    }

    /// Inserts the items in `items` directly after `pos`.
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if any items
    /// in `items` are already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(*m* + log *n*), where *m* is the number of items in
    /// `items`.
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

    /// Inserts `item` directly after `pos`, or at the start of the list if
    /// `pos` is [`None`].
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if `item` is
    /// already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn insert_after_opt(&mut self, pos: Option<L>, item: L) {
        self.insert_after_opt_from(pos, once(item));
    }

    /// Inserts the items in `items` directly after `pos`, or at the start of
    /// the list if `pos` is [`None`].
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if any items
    /// in `items` are already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(*m* + log *n*), where *m* is the number of items in
    /// `items`.
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

    /// Inserts `item` directly before `pos`.
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if `item` is
    /// already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn insert_before(&mut self, pos: L, item: L) {
        self.insert_before_from(pos, once(item));
    }

    /// Inserts the items in `items` directly before `pos`.
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if any items
    /// in `items` are already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(*m* + log *n*), where *m* is the number of items in
    /// `items`.
    pub fn insert_before_from<I>(&mut self, pos: L, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        self.insert_after_opt_from(SkipList::previous(pos), items);
    }

    /// Inserts `item` directly before `pos`, or at the end of the list if
    /// `pos` is [`None`].
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if `item` is
    /// already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn insert_before_opt(&mut self, pos: Option<L>, item: L) {
        self.insert_before_opt_from(pos, once(item));
    }

    /// Inserts the items in `items` directly before `pos`, or at the end of
    /// the list if `pos` is [`None`].
    ///
    /// # Panics
    ///
    /// This method may panic if `pos` is not from this list, or if any items
    /// in `items` are already in a list. Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(*m* + log *n*), where *m* is the number of items in
    /// `items`.
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

    /// Inserts `item` at the start of the list.
    ///
    /// # Panics
    ///
    /// This method may panic if `item` is already in a list. Memory may be
    /// leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
    pub fn push_front(&mut self, item: L) {
        self.push_front_from(once(item));
    }

    /// Inserts the items in `items` at the start of the list.
    ///
    /// # Panics
    ///
    /// This method may panic if any items in `items` are already in a list.
    /// Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Θ(*m* + log *n*), where *m* is the number of items in `items`.
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
            self.insert_after_from(first, iter.chain(once(next)));
        } else {
            debug_assert!(self.root.is_none());
            self.root = Some(Down::Leaf(first.clone()));
            self.insert_after_from(first, iter);
        }
    }

    /// Inserts `item` at the end of the list.
    ///
    /// # Panics
    ///
    /// This method may panic if `item` is already in a list. Memory may be
    /// leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
    pub fn push_back(&mut self, item: L) {
        self.push_back_from(once(item));
    }

    /// Inserts the items in `items` at the end of the list.
    ///
    /// # Panics
    ///
    /// This method may panic if any items in `items` are already in a list.
    /// Memory may be leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Θ(*m* + log *n*), where *m* is the number of items in `items`.
    pub fn push_back_from<I>(&mut self, items: I)
    where
        I: IntoIterator<Item = L>,
    {
        self.insert_after_opt_from(self.last(), items);
    }

    /// Removes `item` from the list.
    ///
    /// # Panics
    ///
    /// This method may panic if `item` is not from this list. Memory may be
    /// leaked in this case.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
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

    /// Updates the [`size`] of an item.
    ///
    /// This method should be used whenever `item` needs to be modified in a
    /// way that could change the value returned by [`L::size`][`size`].
    /// `update` should be a function that performs the modifications.
    ///
    /// [`size`]: LeafRef::size
    ///
    /// # Panics
    ///
    /// This function may panic if `item` is not from this list.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn update<F>(&mut self, item: L, update: F)
    where
        F: FnOnce(),
    {
        let old_size = item.size();
        update();
        let new_size = item.size();
        propagate_update_diff(item, None, old_size, new_size);
    }

    /// Replaces an item with another item.
    ///
    /// `old` should be an item in this list, while `new` should not be in any
    /// list.
    ///
    /// # Panics
    ///
    /// This method may panic if `old` is not from this list, or if `new` is
    /// already in a list.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
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

    /// Gets the first item in the list.
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
    pub fn first(&self) -> Option<L> {
        self.root.clone().map(SkipList::subtree_first)
    }

    /// Gets the last item in the list.
    ///
    /// # Time complexity
    ///
    /// Θ(log *n*).
    pub fn last(&self) -> Option<L> {
        self.root.clone().map(SkipList::subtree_last)
    }

    /// Gets an iterator over the items in the list.
    ///
    /// # Time complexity
    ///
    /// Iteration over the entire list is Θ(*n*).
    pub fn iter(&self) -> Iter<L> {
        Iter(self.first())
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
    L::Options: ListOptions<StoreKeys = Bool<true>>,
{
    /// Inserts an item in a sorted list.
    ///
    /// # Panics
    ///
    /// This method may panic if the list is not sorted.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
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

    /// Finds an item in a sorted list.
    ///
    /// If the item is not in the list, this method returns an [`Err`] value
    /// containing the existing list item that would immediately precede the
    /// desired item if it were to be inserted. This can be used with
    /// [`Self::insert_after_opt`].
    ///
    /// # Panics
    ///
    /// This method may panic if the list is not sorted.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn find<K>(&self, key: &K) -> Result<L, Option<L>>
    where
        K: Ord + ?Sized,
        L: Borrow<K>,
    {
        self.find_with_cmp(|item| item.borrow().cmp(key))
    }

    /// Finds an item in a sorted list with a key type that `L` can't be
    /// borrowed as.
    ///
    /// For this method to yield correct results, `K` and `L` must form a
    /// total order ([`PartialOrd::partial_cmp`] should always return
    /// [`Some`]).
    ///
    /// The return value is the same as for [`Self::find`].
    ///
    /// # Panics
    ///
    /// This method may panic if the list is not sorted, or if `K` and `L` do
    /// not form a total order.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
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

    /// Finds an item in a sorted list using the given comparison function.
    ///
    /// `cmp` checks whether its argument is less than, equal to, or greater
    /// than the desired item. Thus, the argument provided to `cmp` is
    /// logically the *left-hand* side of the comparison.
    ///
    /// The return value is the same as for [`Self::find`].
    ///
    /// # Panics
    ///
    /// This method may panic if the list is not sorted, or if `cmp` returns
    /// results inconsistent with the total order on `L`.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn find_with_cmp<F>(&self, cmp: F) -> Result<L, Option<L>>
    where
        F: Fn(&L) -> Ordering,
    {
        SkipList::subtree_find(cmp, self.root.clone().ok_or(None)?)
    }
}

impl<L> SkipList<L>
where
    L: LeafRef,
    L::Options: ListOptions<StoreKeys = Bool<true>>,
{
    fn subtree_find<F>(cmp: F, first_child: Down<L>) -> Result<L, Option<L>>
    where
        F: Fn(&L) -> Ordering,
    {
        let mut node = first_child;
        #[cfg(debug_assertions)]
        let mut first = true;
        loop {
            // These variables are only used in their respective loops, but
            // defining them outside of the `match` reduces indentation.
            let mut prev_leaf: Option<L> = None;
            let mut prev_internal: Option<InternalNodeRef<L>> = None;
            node = match node {
                Down::Leaf(mut node) => loop {
                    println!("{:?}", cmp(&node));
                    match cmp(&node) {
                        Ordering::Less => {}
                        Ordering::Equal => return Ok(node),
                        Ordering::Greater => {
                            #[cfg(debug_assertions)]
                            debug_assert!(first || prev_leaf.is_some());
                            return Err(prev_leaf);
                        }
                    }
                    if let Some(next) = node.next_sibling() {
                        prev_leaf = Some(node);
                        node = next;
                    } else {
                        return Err(Some(node));
                    }
                },
                Down::Internal(mut node) => loop {
                    let key = node.key().unwrap();
                    match cmp(&key) {
                        Ordering::Less => {}
                        Ordering::Equal => return Ok(key),
                        Ordering::Greater => {
                            #[cfg(debug_assertions)]
                            debug_assert!(first || prev_internal.is_some());
                            break prev_internal.ok_or(None)?.down().unwrap();
                        }
                    }
                    if let Some(next) = node.next_sibling() {
                        prev_internal = Some(node);
                        node = next;
                    } else {
                        break node.down().unwrap();
                    }
                },
            };
            #[cfg(debug_assertions)]
            {
                first = false;
            }
        }
    }

    /// Finds an item in a sorted list, at or after a given item.
    ///
    /// If the desired item occurs at or after `start`, or is not present in
    /// the list but would be ordered after `start`, this method returns the
    /// same result as [`Self::find`]. Otherwise, <code>[Err]\([None])</code>
    /// is returned.
    ///
    /// # Panics
    ///
    /// This method may panic if the list is not sorted.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn find_after<K>(start: L, key: &K) -> Result<L, Option<L>>
    where
        K: Ord + ?Sized,
        L: Borrow<K>,
    {
        Self::find_after_with_cmp(start, |item| item.borrow().cmp(key))
    }

    /// Finds an item in a sorted list, at or after a given item, with a key
    /// type that `L` can't be borrowed as.
    ///
    /// The return value is the same as for [`Self::find_after`]. This method
    /// is to [`Self::find_after`] what [`Self::find_with`] is to
    /// [`Self::find`].
    ///
    /// For this method to yield correct results, `K` and `L` must form a
    /// total order ([`PartialOrd::partial_cmp`] should always return
    /// [`Some`]).
    ///
    /// # Panics
    ///
    /// This method may panic if the list is not sorted, or if `K` and `L` do
    /// not form a total order.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn find_after_with<K>(start: L, key: &K) -> Result<L, Option<L>>
    where
        K: ?Sized,
        L: PartialOrd<K>,
    {
        Self::find_after_with_cmp(start, |item| {
            item.partial_cmp(key).unwrap_or_else(
                #[cold]
                || panic!("`partial_cmp` returned `None`"),
            )
        })
    }

    /// Finds an item in a sorted list, at or after a given item, using the
    /// given comparison function.
    ///
    /// The return value is the same as for [`Self::find_after`]. This method
    /// is to [`Self::find_after`] what [`Self::find_with_cmp`] is to
    /// [`Self::find`].
    ///
    /// `cmp` checks whether its argument is less than, equal to, or greater
    /// than the desired item. Thus, the argument provided to `cmp` is
    /// logically the *left-hand* side of the comparison.
    ///
    /// # Panics
    ///
    /// This method may panic if the list is not sorted, or if `cmp` returns
    /// results inconsistent with the total order on `L`.
    ///
    /// # Time complexity
    ///
    /// Worst-case Θ(log *n*).
    pub fn find_after_with_cmp<F>(start: L, cmp: F) -> Result<L, Option<L>>
    where
        F: Fn(&L) -> Ordering,
    {
        let mut leaf = start;
        let mut prev = None;
        let mut internal = loop {
            match cmp(&leaf) {
                Ordering::Less => {}
                Ordering::Equal => return Ok(leaf),
                Ordering::Greater => return Err(prev),
            }
            match NodeRef::next(&leaf) {
                Some(Next::Sibling(next)) => {
                    prev = Some(leaf);
                    leaf = next;
                }
                Some(Next::Parent(node)) => break node,
                None => return Err(Some(leaf)),
            }
        };

        let mut leaf_is_last = true;
        let down = loop {
            match internal.next() {
                Some(Next::Sibling(next)) => {
                    let key = next.key().unwrap();
                    let ord = cmp(&key);
                    match ord {
                        Ordering::Less => {}
                        Ordering::Equal => return Ok(key),
                        Ordering::Greater => break internal.down().unwrap(),
                    }
                    internal = next;
                    leaf_is_last = false;
                }
                Some(Next::Parent(node)) => {
                    internal = node;
                    continue;
                }
                None if leaf_is_last => return Err(Some(leaf)),
                None => break internal.down().unwrap(),
            }
        };
        Self::subtree_find(cmp, down)
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

impl<L, A> Extend<L> for SkipList<L, A>
where
    L: LeafRef,
    A: Allocator,
{
    /// Equivalent to [`Self::push_back_from`].
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = L>,
    {
        self.push_back_from(iter);
    }
}

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

//! Skip list iterators.

use super::{LeafRef, SkipList};
use crate::allocator::Allocator;
use core::iter::FusedIterator;

/// An iterator over the items in a [`SkipList`].
pub struct Iter<L>(pub(super) Option<L>);

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

/// An owning iterator over the items in a [`SkipList`].
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

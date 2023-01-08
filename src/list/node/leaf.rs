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

use super::{AllocItem, Down, InternalNodeRef, Next, NodeRef};
use crate::options::{LeafSize, ListOptions, StoreKeysPriv};
use core::ops::{AddAssign, SubAssign};
use core::ptr::NonNull;

type StoreKeys<L> = <<L as LeafRef>::Options as ListOptions<L>>::StoreKeys;
pub type Key<L> = <StoreKeys<L> as StoreKeysPriv<L>>::Key;

/// # Safety
///
/// * `Self` must not be [`Send`] or [`Sync`].
///
/// * [`Self::next`] must initially return [`None`] until [`Self::set_next`] is
///   called.
///
/// * After [`Self::set_next`] is called (with parameter `params`), future
///   calls to [`Self::next`] must return a value identical to `params.get().1`
///   until the next call to [`Self::set_next`].
///
/// * Because this type is conceptually a reference, clones produced through
///   [`Clone::clone`] must behave identically to the original object. In
///   particular, if an operation is performed on an object `s` of type `Self`,
///   all clones of `s` (transitively and symmetrically) must behave as if that
///   same operation were performed on them.
pub unsafe trait LeafRef: Clone {
    type Options: ListOptions<Self>;
    const FANOUT: usize = 8;

    fn next(&self) -> Option<LeafNext<Self>>;
    fn set_next(params: SetNextParams<'_, Self>);
    fn size(&self) -> LeafSize<Self> {
        Default::default()
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

    fn size(&self) -> LeafSize<Self> {
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
        StoreKeys::<Self>::as_key(self)
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

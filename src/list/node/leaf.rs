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

/// Represents a *reference* to an item, or “leaf”, in a [`SkipList`].
///
/// # Safety
///
/// * `Self` must not be [`Send`] or [`Sync`]. For more information on
///   concurrency, see the [Concurrency section] in [`SkipList`]'s
///   documentation.
///
/// * [`Self::next`] must initially return [`None`] until [`Self::set_next`] is
///   called.
///
/// * After [`Self::set_next`] is called (with parameter `params`), future
///   calls to [`Self::next`] must return a value identical to
///   <code>[params.get()].1</code> until the next call to [`Self::set_next`].
///
/// * Because this type is conceptually a reference, clones produced through
///   [`Clone::clone`] must behave identically to the original object. In
///   particular, if an operation is performed on an object `s` of type `Self`,
///   all clones of `s` (transitively and symmetrically) must behave as if that
///   same operation were performed on them.
///
/// [`SkipList`]: crate::SkipList
/// [Concurrency section]: crate::SkipList#concurrency
/// [params.get()]: SetNextParams::get
pub unsafe trait LeafRef: Clone {
    /// Options that configure the list; see [`ListOptions`] and [`Options`].
    ///
    /// [`Options`]: crate::Options
    type Options: ListOptions<Self>;

    /// The maximum number of children each node in the list can have.
    ///
    /// If this is less than 3, it will be treated as 3.
    const FANOUT: usize = 8;

    /// Gets the item/data that follows this leaf.
    ///
    /// Leaf items should be able to store the item provided to
    /// [`Self::set_next`] and return it from this method.
    fn next(&self) -> Option<LeafNext<Self>>;

    /// Sets the item/data that follows this leaf.
    ///
    /// `params` contains the parameters for what would typically be:
    ///
    /// ```
    /// # struct LeafNext<L>(L);
    /// unsafe trait LeafRef: Clone {
    ///     fn set_next(&self, next: Option<LeafNext<Self>>);
    /// }
    /// ```
    ///
    /// You can use [`params.get()`][params.get()] to obtain the inner
    /// parameters; see [`SetNextParams`] for more information.
    ///
    /// This method should store <code>[params.get()].1</code> somewhere so
    /// that it can be returned by [`Self::next`].
    ///
    /// [params.get()]: SetNextParams::get
    fn set_next(params: SetNextParams<'_, Self>);

    /// Gets the size of this item.
    ///
    /// By default, this method returns [`Default::default()`], which should be
    /// a zero-like value.
    fn size(&self) -> LeafSize<Self> {
        Default::default()
    }
}

/// The item/data that can be stored and retrieved with [`LeafRef::set_next`]
/// and [`LeafRef::next`].
///
/// This can be stored directly by the type that implements [`LeafRef`], but
/// it also makes certain details available to enable more efficient
/// representations.
///
/// The pointer in [`Self::Data`] is guaranteed to be aligned to at least the
/// alignment of <code>[L::Options]::[Align]</code>.
///
/// [L::Options]: LeafRef::Options
/// [Align]: ListOptions::Align
#[derive(Clone, Debug)]
pub enum LeafNext<L: LeafRef> {
    /// A leaf item.
    Leaf(L),
    /// Arbitrary data that should be stored.
    Data(NonNull<AllocItem<L>>),
}

/// Parameters for [`LeafRef::set_next`].
///
/// This type represents the parameters for what would typically be:
///
/// ```
/// # struct LeafNext<L>(L);
/// unsafe trait LeafRef: Clone {
///     fn set_next(&self, next: Option<LeafNext<Self>>);
/// }
/// ```
///
/// But for safety reasons, a separate type must be provided (this is so
/// implementations of [`LeafRef::set_next`] can't call [`set_next`] on another
/// [`LeafRef`].
///
/// [`set_next`]: LeafRef::set_next
///
/// You can use [`Self::get`] to obtain the inner parameters.
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

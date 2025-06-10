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

use super::{AllocItem, Down, InternalNodeRef, Key, Next, NodeRef};
use crate::options::{LeafSize, ListOptions};
use core::ops::{AddAssign, Deref, SubAssign};
use core::ptr::NonNull;

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
/// * After [`Self::set_next`] is called (with some `next` parameter), future
///   calls to [`Self::next`] must return a value identical to the `next`
///   parameter previously provided to [`Self::set_next`] until the next call
///   to [`Self::set_next`].
///
/// * Because this type is conceptually a reference, clones produced through
///   [`Clone::clone`] must behave identically to the original object. In
///   particular, if an operation is performed on an object `s` of type `Self`,
///   all clones of `s` (transitively and symmetrically) must behave as if that
///   same operation were performed on them.
///
/// [`SkipList`]: crate::SkipList
/// [Concurrency section]: crate::SkipList#concurrency
pub unsafe trait LeafRef: Clone {
    /// Options that configure the list; see [`ListOptions`] and [`Options`].
    ///
    /// [`Options`]: crate::Options
    type Options: ListOptions;

    /// Gets the item/data that follows this leaf.
    ///
    /// Leaf items should be able to store the item provided to
    /// [`Self::set_next`] and return it from this method.
    fn next(&self) -> Option<LeafNext<Self>>;

    /// Sets the item/data that follows this leaf.
    ///
    /// For safety reasons,[^1] instead of a `&self` parameter, this function
    /// takes a value of type <code>[This]\<[&][r][Self]></code>. This type
    /// implements <code>[Deref]\<[Target](Deref::Target) = [Self]></code>, so
    /// it can be used similarly to <code>[&][r][Self]</code>.
    ///
    /// This method should store `next` somewhere so that it can be returned
    /// by [`Self::next`].
    ///
    /// [^1]: This prevents an implementation of [`set_next`] from calling
    /// [`set_next`] on other [`LeafRef`]s.
    ///
    /// [r]: reference
    /// [`set_next`]: Self::set_next
    fn set_next(this: This<&'_ Self>, next: Option<LeafNext<Self>>);

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
#[derive(Clone, Debug)]
pub enum LeafNext<L: LeafRef> {
    /// A leaf item.
    Leaf(L),

    /// Arbitrary data that should be stored.
    ///
    /// The pointer is guaranteed to be properly aligned, which in the case of
    /// [`AllocItem<L>`] means its alignment is at least as large as the
    /// alignment of <code>L::[Options]::[Align]</code>.
    ///
    /// [Options]: LeafRef::Options
    /// [Align]: ListOptions::Align
    Data(NonNull<AllocItem<L>>),
}

/// A wrapper around a method's `self` parameter.
///
/// Instead of `&self`, [`LeafRef::set_next`] takes a parameter of type
/// <code>[This]\<[&][r][Self](LeafRef)></code> to enforce certain safety
/// requirements; see its documentation for more information.
///
/// [r]: reference
pub struct This<T>(T);

impl<T> Deref for This<&'_ T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
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
        LeafRef::set_next(
            This(self),
            next.map(|next| match next {
                Next::Sibling(node) => LeafNext::Leaf(node),
                Next::Parent(node) => LeafNext::Data(node.as_ptr()),
            }),
        );
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
        use crate::options::StoreKeysPriv;
        super::StoreKeys::<Self>::as_key(self)
    }
}

pub trait LeafExt: LeafRef {
    fn set_next_leaf(&self, next: Option<LeafNext<Self>>) {
        Self::set_next(This(self), next);
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

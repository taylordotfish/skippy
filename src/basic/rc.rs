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

use super::BasicLeaf;
use super::options::BasicOptions;
use crate::options::{LeafSize, TypedOptions};
use crate::{LeafNext, LeafRef, This};
use alloc::rc::Rc;
use core::cell::Cell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

/// Stores data of type `T`. <code>[Rc]\<[RcLeaf]\<T>></code> implements
/// [`LeafRef`] and can be used with [`SkipList`](crate::SkipList).
#[repr(align(2))]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct RcLeaf<T> {
    data: T,
    next: Cell<Option<TaggedPtr<Self, 1>>>,
}

impl<T> RcLeaf<T> {
    /// Creates a new [`RcLeaf<T>`].
    pub fn new(data: T) -> Self {
        Self {
            data,
            next: Cell::default(),
        }
    }

    /// Takes ownership of the inner value of type `T`.
    pub fn into_inner(this: Self) -> T {
        this.data
    }
}

impl<T> From<T> for RcLeaf<T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<T> Deref for RcLeaf<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<T> DerefMut for RcLeaf<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<T: fmt::Debug> fmt::Debug for RcLeaf<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("RcLeaf")
            .field("addr", &(self as *const _))
            .field("data", &self.data)
            .field("next", &self.next.get())
            .finish()
    }
}

// SAFETY:
// * `Rc` is not `Send` or `Sync`.
// * `Self::next` will initially return `None` because `RcLeaf::next` is
//   initialized as `None`.
// * `Self::set_next` stores its argument in `RcLeaf::next` and is the only
//   function that modifies that field. `Self::next` retrieves the value
//   appropriately.
// * Clones of `Rc` behave like the original pointer.
unsafe impl<T: BasicLeaf> LeafRef for Rc<RcLeaf<T>> {
    type Options = TypedOptions<
        <T::Options as BasicOptions>::SizeType,
        <T::Options as BasicOptions>::StoreKeys,
        <T::Options as BasicOptions>::Fanout,
        RcLeaf<T>, /* Align */
    >;

    fn next(&self) -> Option<LeafNext<Self>> {
        let (ptr, tag) = self.next.get()?.get();
        Some(match tag {
            // SAFETY: A tag of 0 corresponds to a leaf pointer.
            0 => LeafNext::Leaf(unsafe { Rc::from_raw(ptr.as_ptr()) }),
            _ => LeafNext::Data(ptr.cast()),
        })
    }

    fn set_next(this: This<&'_ Self>, next: Option<LeafNext<Self>>) {
        this.next.set(next.map(|n| match n {
            LeafNext::Leaf(leaf) => TaggedPtr::new(
                // SAFETY: `Rc::into_raw` always returns non-null pointers.
                unsafe { NonNull::new_unchecked(Rc::into_raw(leaf) as _) },
                0,
            ),
            LeafNext::Data(data) => TaggedPtr::new(data.cast(), 1),
        }))
    }

    fn size(&self) -> LeafSize<Self> {
        self.data.size()
    }
}

#[cfg(skippy_debug)]
impl<T> crate::list::debug::LeafDebug for Rc<RcLeaf<T>>
where
    T: BasicLeaf + fmt::Debug,
{
    type Id = *const RcLeaf<T>;

    fn id(&self) -> Self::Id {
        Rc::as_ptr(self)
    }

    fn fmt_data(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.data)
    }
}

#[cfg(any(doc, doctest))]
/// <code>[Rc]\<[RcLeaf]></code> cannot implement [`Send`] or [`Sync`], as this
/// would make it unsound to implement [`LeafRef`].
///
/// ```
/// use skippy::basic::RcLeaf;
/// struct Test<T = std::rc::Rc<RcLeaf<u8>>>(T);
/// ```
///
/// ```compile_fail
/// use skippy::basic::RcLeaf;
/// struct Test<T: Send = std::rc::Rc<RcLeaf<u8>>>(T);
/// ```
///
/// ```compile_fail
/// use skippy::basic::RcLeaf;
/// struct Test<T: Sync = std::rc::Rc<RcLeaf<u8>>>(T);
/// ```
mod leaf_is_not_send_sync {}

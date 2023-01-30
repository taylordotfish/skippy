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

//! Basic skip list options.

#[cfg(doc)]
use super::BasicLeaf;
pub use crate::options::{Bool, NoSize};
use core::marker::PhantomData;
use core::ops::{AddAssign, SubAssign};

mod sealed {
    pub trait Sealed {}
}

/// Options trait for [`BasicLeaf::Options`].
///
/// This is a sealed trait; use the [`Options`] type, which implements this
/// trait.
pub trait BasicOptions: sealed::Sealed {
    /// The type that represents the size of an item in a [`SkipList`].
    ///
    /// See [`ListOptions::SizeType`](crate::ListOptions::SizeType).
    ///
    /// [`SkipList`]: crate::SkipList
    type SizeType: Clone + Default + Eq + AddAssign + SubAssign;

    /// Whether or not to store keys representing items in the internal parts
    /// of the list.
    ///
    /// See [`ListOptions::StoreKeys`](crate::ListOptions::StoreKeys).
    type StoreKeys;
}

/// Options for [`BasicLeaf::Options`].
///
/// This type implements [`BasicOptions`]. Type and const parameters correspond
/// to associated types in [`BasicOptions`] as follows; see those associated
/// types for documentation:
///
/// Parameter    | Associated type
/// ------------ | ---------------------------
/// `SizeType`   | [`BasicOptions::SizeType`]
/// `STORE_KEYS` | [`BasicOptions::StoreKeys`]
#[rustfmt::skip]
pub type Options<
    SizeType = NoSize,
    const STORE_KEYS: bool = false,
> = TypedOptions<
    SizeType,
    Bool<STORE_KEYS>,
>;

/// Like [`Options`], but uses types instead of const parameters.
///
/// [`Options`] is actually a type alias of this type.
#[allow(clippy::type_complexity)]
#[rustfmt::skip]
pub struct TypedOptions<
    SizeType = NoSize,
    StoreKeys = Bool<false>,
>(PhantomData<fn() -> (
    SizeType,
    StoreKeys,
)>);

#[rustfmt::skip]
impl<
    SizeType,
    StoreKeys,
> sealed::Sealed for TypedOptions<
    SizeType,
    StoreKeys,
> {}

#[rustfmt::skip]
impl<
    SizeType: Clone + Default + Eq + AddAssign + SubAssign,
    StoreKeys,
> BasicOptions for TypedOptions<
    SizeType,
    StoreKeys,
> {
    type SizeType = SizeType;
    type StoreKeys = StoreKeys;
}

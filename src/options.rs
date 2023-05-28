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

//! Skip list options.

use crate::LeafRef;
#[cfg(doc)]
use crate::{LeafNext, SkipList};
use core::convert::Infallible;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{AddAssign, SubAssign};

/// Represents a [`bool`].
pub struct Bool<const B: bool>(());

/// Represents a [`usize`].
pub struct Usize<const N: usize>(());

mod detail {
    pub trait StoreKeysPriv {
        type Key<T: Clone>: Clone;

        fn as_key<T: Clone>(_value: &T) -> Option<Self::Key<T>> {
            None
        }
    }

    pub trait FanoutPriv {
        const VALUE: usize;
    }
}

pub(crate) use detail::*;

/// Trait bound on [`ListOptions::StoreKeys`].
pub trait StoreKeys: StoreKeysPriv {}

impl StoreKeys for Bool<false> {}
impl StoreKeysPriv for Bool<false> {
    type Key<T: Clone> = Infallible;
}

impl StoreKeys for Bool<true> {}
impl StoreKeysPriv for Bool<true> {
    type Key<T: Clone> = T;

    fn as_key<T: Clone>(value: &T) -> Option<T> {
        Some(value.clone())
    }
}

/// Trait bound on [`ListOptions::Fanout`].
pub trait Fanout: FanoutPriv {}

impl<const N: usize> Fanout for Usize<N> {}
impl<const N: usize> FanoutPriv for Usize<N> {
    const VALUE: usize = N;
}

/// A no-op, zero-sized size type for lists whose items don't need a notion of
/// size.
///
/// This type can be used as the `SizeType` parameter in [`Options`], which
/// corresponds to [`ListOptions::SizeType`].
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct NoSize;

impl fmt::Debug for NoSize {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "∅")
    }
}

impl AddAssign for NoSize {
    fn add_assign(&mut self, _rhs: Self) {}
}

impl SubAssign for NoSize {
    fn sub_assign(&mut self, _rhs: Self) {}
}

mod sealed {
    pub trait Sealed {}
}

/// Options trait for [`LeafRef::Options`].
///
/// This is a sealed trait; use the [`Options`] type, which implements this
/// trait.
pub trait ListOptions: sealed::Sealed {
    /// The type that represents the size of an item in a [`SkipList`].
    ///
    /// This could be something simple like [`usize`], or something more
    /// complex. For correctness, it should conceptually represent an unsigned
    /// integer or collection of unsigned integers---returning negative values
    /// from [`LeafRef::size`] will produce incorrect results.
    ///
    /// *Default:* [`NoSize`]
    type SizeType: Clone + Default + Eq + AddAssign + SubAssign;

    /// Whether or not to store keys representing items in the internal parts
    /// of the list.
    ///
    /// This enables methods like [`SkipList::find`] and [`SkipList::insert`]
    /// to be used on sorted lists.
    ///
    /// *Default:* false
    type StoreKeys: StoreKeys;

    /// The maximum number of children each node in the list can have.
    ///
    /// If this is less than 3, it will be treated as 3.
    ///
    /// *Default:* 8
    type Fanout: Fanout;

    /// The minimum alignment that [`LeafNext::Data`] should have. This can
    /// help enable certain hacks like [tagged pointers].
    ///
    /// Specifically, the pointer in [`LeafNext::Data`] will be aligned to at
    /// least the alignment of this type.
    ///
    /// If you have no special alignment requirements, this can be
    /// [`()`](unit).
    ///
    /// *Default*: [`()`]
    ///
    /// [tagged pointers]: tagged_pointer
    type Align;
}

/// Alias of <code>[LeafRef::Options]::[SizeType]</code>.
///
/// [SizeType]: ListOptions::SizeType
pub type LeafSize<L> = <<L as LeafRef>::Options as ListOptions>::SizeType;

/// Options for [`LeafRef::Options`].
///
/// This type implements [`ListOptions`]. Type and const parameters correspond
/// to associated types in [`ListOptions`] as follows; see those associated
/// types for documentation:
///
/// Parameter    | Associated type
/// ------------ | --------------------------
/// `SizeType`   | [`ListOptions::SizeType`]
/// `STORE_KEYS` | [`ListOptions::StoreKeys`]
/// `FANOUT`     | [`ListOptions::Fanout`]
/// `Align`      | [`ListOptions::Align`]
#[rustfmt::skip]
pub type Options<
    SizeType = NoSize,
    const STORE_KEYS: bool = false,
    const FANOUT: usize = 8,
    Align = (),
> = TypedOptions<
    SizeType,
    Bool<STORE_KEYS>,
    Usize<FANOUT>,
    Align,
>;

/// Like [`Options`], but uses types instead of const parameters.
///
/// [`Options`] is actually a type alias of this type.
#[allow(clippy::type_complexity)]
#[rustfmt::skip]
pub struct TypedOptions<
    SizeType = NoSize,
    StoreKeys = Bool<false>,
    Fanout = Usize<8>,
    Align = (),
>(PhantomData<fn() -> (
    SizeType,
    StoreKeys,
    Fanout,
    Align,
)>);

#[rustfmt::skip]
impl<
    SizeType,
    StoreKeys,
    Fanout,
    Align,
> sealed::Sealed for TypedOptions<
    SizeType,
    StoreKeys,
    Fanout,
    Align,
> {}

#[rustfmt::skip]
impl<
    SizeType: Clone + Default + Eq + AddAssign + SubAssign,
    StoreKeys: self::StoreKeys,
    Fanout: self::Fanout,
    Align,
> ListOptions for TypedOptions<
    SizeType,
    StoreKeys,
    Fanout,
    Align,
> {
    type SizeType = SizeType;
    type StoreKeys = StoreKeys;
    type Fanout = Fanout;
    type Align = Align;
}

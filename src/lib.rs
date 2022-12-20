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

#![cfg_attr(not(any(feature = "std", all(test, skippy_debug))), no_std)]
#![cfg_attr(has_allocator_api, feature(allocator_api))]
#![deny(unsafe_op_in_unsafe_fn)]

//! A highly flexible, non-amortized worst-case O(log n) intrusive skip list.
//!
//! The skip list can be used both as an ordered sequence (allowing it to be
//! used like a set or map) and as an unordered sequence (allowing it to be
//! used like a vector/dynamic array). Elements support an optional notion of
//! “size”, allowing insertions, removals, and lookups by index, as well as,
//! due to the intrusive nature of the skip list, the ability to query an
//! element’s index.

#[cfg(not(any(feature = "allocator_api", feature = "allocator-fallback")))]
compile_error!("allocator_api or allocator-fallback must be enabled");

extern crate alloc;

#[cfg(feature = "allocator_api")]
use alloc::alloc as allocator;

#[cfg(not(feature = "allocator_api"))]
use allocator_fallback as allocator;

pub mod basic;
mod list;
#[cfg(test)]
mod tests;

#[cfg(skippy_debug)]
pub use list::debug;
pub use list::{AllocItem, LeafNext, LeafRef, SetNextParams};
pub use list::{IntoIter, Iter, SkipList};
pub use list::{NoSize, StoreKeys, StoreKeysOption};

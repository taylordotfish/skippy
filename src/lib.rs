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

#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(has_allocator_api, feature(allocator_api))]
#![deny(unsafe_op_in_unsafe_fn)]
#![warn(clippy::undocumented_unsafe_blocks)]
// crate doc:
#![doc = include_str!("common-readme.md")]
//!
//! [fanout]: ListOptions::Fanout
//! [`Allocator`]: alloc::alloc::Allocator
//! [allocator-fallback]: allocator_fallback

#[cfg(not(any_allocator_api))]
compile_error!("allocator_api or allocator-fallback must be enabled");

extern crate alloc;

#[cfg(feature = "allocator_api")]
use alloc::alloc as allocator;

#[cfg(not(feature = "allocator_api"))]
#[cfg(feature = "allocator-fallback")]
use allocator_fallback as allocator;

pub mod basic;
mod list;
pub mod options;
mod persistent_alloc;

#[cfg(skippy_debug)]
pub use list::debug;
pub use list::{AllocItem, LeafNext, LeafRef, SkipList, This, iter};
pub use options::{LeafSize, ListOptions, NoSize, Options};
use persistent_alloc::PersistentAlloc;

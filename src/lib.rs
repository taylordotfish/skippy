#![cfg_attr(not(any(feature = "std", test)), no_std)]
#![cfg_attr(feature = "allocator_api", feature(allocator_api))]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(not(any(feature = "allocator_api", feature = "allocator-fallback")))]
compile_error!("allocator_api or allocator-fallback must be enabled");

#[cfg(feature = "allocator_api")]
use alloc::alloc::{Allocator, Global};

#[cfg(not(feature = "allocator_api"))]
use allocator_fallback::{Allocator, Global};

extern crate alloc;

pub mod basic;
mod list;
#[cfg(test)]
mod tests;

#[cfg(skip_list_debug)]
#[allow(unused_imports)]
pub use list::debug;
pub use list::{AllocItem, LeafNext, LeafRef, SetNextParams};
pub use list::{IntoIter, Iter, SkipList};
pub use list::{NoSize, StoreKeys, StoreKeysOption};

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

use crate::allocator::{AllocError, Allocator};
use alloc::alloc::Layout;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use core::ptr::NonNull;

pub struct PersistentAlloc<A>(ManuallyDrop<A>);

impl<A: Allocator> PersistentAlloc<A> {
    /// Creates a new [`PersistentAlloc`] with `alloc` as the inner allocator.
    ///
    /// The inner allocator can be accessed via this type's implementation of
    /// [`Deref`].
    pub fn new(alloc: A) -> Self
    where
        A: 'static,
    {
        Self(ManuallyDrop::new(alloc))
    }

    /// Drops the inner allocator.
    ///
    /// # Safety
    ///
    /// * Every block of memory allocated by the inner allocator must have been
    ///   deallocated with [`Allocator::deallocate`].
    /// * This [`PersistentAlloc`] must not be used (including being
    ///   dereferenced) after this method is called.
    pub unsafe fn drop(&mut self) {
        // SAFETY: Checked by caller.
        unsafe {
            ManuallyDrop::drop(&mut self.0);
        }
    }
}

impl<A> Deref for PersistentAlloc<A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// SAFETY: Simply forwards to the inner allocator's implementation.
/// Forwards to the inner allocator.
unsafe impl<A: Allocator> Allocator for PersistentAlloc<A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.0.allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Checked by caller.
        unsafe {
            self.0.deallocate(ptr, layout);
        }
    }
}

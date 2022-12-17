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

#[cfg(feature = "std")]
use core::cell::Cell;

#[cfg(not(feature = "std"))]
use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "std")]
thread_local! {
    static CAN_SAFELY_DESTROY: Cell<bool> = Cell::new(true);
}

#[cfg(not(feature = "std"))]
static CAN_SAFELY_DESTROY: AtomicBool = AtomicBool::new(true);

pub fn can_safely_destroy() -> bool {
    #[cfg(feature = "std")]
    return CAN_SAFELY_DESTROY.with(Cell::get);
    #[cfg(not(feature = "std"))]
    return CAN_SAFELY_DESTROY.load(Ordering::Relaxed);
}

pub fn set_cannot_safely_destroy() {
    #[cfg(feature = "std")]
    CAN_SAFELY_DESTROY.with(|c| c.set(false));
    #[cfg(not(feature = "std"))]
    CAN_SAFELY_DESTROY.store(false, Ordering::Relaxed);
}

pub struct SetUnsafeOnDrop;

impl Drop for SetUnsafeOnDrop {
    fn drop(&mut self) {
        set_cannot_safely_destroy();
    }
}

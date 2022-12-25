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

use super::BasicLeaf;
use crate::{LeafNext, LeafRef, SetNextParams, StoreKeysOption};
use core::cell::Cell;
use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use tagged_pointer::TaggedPtr;

#[repr(align(2))]
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct RefLeaf<'a, T> {
    data: T,
    next: Cell<Option<TaggedPtr<Self, 1>>>,
    phantom: PhantomData<Cell<&'a Self>>,
}

impl<'a, T> RefLeaf<'a, T> {
    pub fn new(data: T) -> Self {
        Self {
            data,
            next: Cell::default(),
            phantom: PhantomData,
        }
    }

    pub fn into_inner(this: Self) -> T {
        this.data
    }
}

impl<'a, T> From<T> for RefLeaf<'a, T> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T> Deref for RefLeaf<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.data
    }
}

impl<'a, T> DerefMut for RefLeaf<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

impl<'a, T> fmt::Debug for RefLeaf<'a, T>
where
    T: BasicLeaf + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("RefLeaf")
            .field("addr", &(self as *const _))
            .field("data", &self.data)
            .field("next", &self.next.get())
            .finish()
    }
}

unsafe impl<'a, T> LeafRef for &RefLeaf<'a, T>
where
    T: BasicLeaf,
    T::StoreKeys: StoreKeysOption<Self>,
{
    const FANOUT: usize = T::FANOUT;
    type Size = T::Size;
    type StoreKeys = T::StoreKeys;
    type Align = RefLeaf<'a, T>;

    fn next(&self) -> Option<LeafNext<Self>> {
        self.next.get().map(|p| match p.get() {
            // SAFETY: A tag of 0 corresponds to a leaf pointer.
            (ptr, 0) => LeafNext::Leaf(unsafe { ptr.as_ref() }),
            (ptr, _) => LeafNext::Data(ptr.cast()),
        })
    }

    fn set_next(params: SetNextParams<'_, Self>) {
        let (this, next) = params.get();
        this.next.set(next.map(|n| match n {
            LeafNext::Leaf(leaf) => TaggedPtr::new(NonNull::from(leaf), 0),
            LeafNext::Data(data) => TaggedPtr::new(data.cast(), 1),
        }))
    }

    fn size(&self) -> Self::Size {
        self.data.size()
    }
}

#[cfg(skippy_debug)]
impl<'a, 'b, T> crate::list::debug::LeafDebug for &'b RefLeaf<'a, T>
where
    T: BasicLeaf + fmt::Debug,
    T::StoreKeys: StoreKeysOption<Self>,
{
    type Id = *const RefLeaf<'a, T>;
    type Data = &'b T;

    fn id(&self) -> Self::Id {
        *self as _
    }

    fn data(&self) -> &'b T {
        &self.data
    }
}

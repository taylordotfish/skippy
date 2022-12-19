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

use core::convert::Infallible;

pub trait StoreKeysOptionPriv<T> {
    type Key: Clone;

    fn as_key(value: &T) -> Option<Self::Key> {
        let _ = value;
        None
    }
}

pub struct StoreKeys<const B: bool>;

impl<T: Clone> StoreKeysOptionPriv<T> for StoreKeys<true> {
    type Key = T;

    fn as_key(value: &T) -> Option<T> {
        Some(value.clone())
    }
}

impl<T> StoreKeysOptionPriv<T> for StoreKeys<false> {
    type Key = Infallible;
}

use StoreKeysOptionPriv as Sealed;
pub trait StoreKeysOption<T>: Sealed<T> {}

impl<T: Clone> StoreKeysOption<T> for StoreKeys<true> {}
impl<T> StoreKeysOption<T> for StoreKeys<false> {}

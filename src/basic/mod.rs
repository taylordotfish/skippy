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

pub mod options;
mod rc;
mod reference;

pub use options::{BasicOptions, Options};
pub use rc::RcLeaf;
pub use reference::RefLeaf;

pub trait BasicLeaf {
    type Options: BasicOptions;
    const FANOUT: usize = 8;

    fn size(&self) -> <Self::Options as BasicOptions>::SizeType {
        Default::default()
    }
}

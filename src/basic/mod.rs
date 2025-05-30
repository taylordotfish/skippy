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

//! “Basic” implementations of [`LeafRef`] that store data of a given type.
//!
//! This module provides two types that, when wrapped in the appropriate
//! reference-like type, implement [`LeafRef`]:
//!
//! * [`RefLeaf`], where <code>[&][r][RefLeaf]</code> implements [`LeafRef`].
//! * [`RcLeaf`], where <code>[Rc]\<[RcLeaf]\></code> implements [`LeafRef`].
//!
//! [r]: prim@reference
//! [Rc]: alloc::rc::Rc

#[cfg(doc)]
use crate::LeafRef;

pub mod options;
mod rc;
mod reference;

pub use options::{BasicOptions, Options};
pub use rc::RcLeaf;
pub use reference::RefLeaf;

/// In order to use the basic implementations of [`LeafRef`] in this module,
/// the type of the stored data must implement this trait.
pub trait BasicLeaf {
    /// Options that configure the list; see [`BasicOptions`] and [`Options`].
    type Options: BasicOptions;

    /// Gets the size of this item.
    ///
    /// By default, this method returns [`Default::default()`], which should be
    /// a zero-like value.
    fn size(&self) -> <Self::Options as BasicOptions>::SizeType {
        Default::default()
    }
}

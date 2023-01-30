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

use crate::options::LeafSize;

pub mod internal;
pub mod leaf;

pub use internal::{AllocItem, InternalNodeRef};
pub use leaf::{Key, LeafExt, LeafNext, LeafRef, SizeExt, This};

pub trait NodeRef: Clone {
    type Leaf: LeafRef;
    fn next(&self) -> Option<Next<Self>>;
    fn set_next(&self, next: Option<Next<Self>>);
    fn size(&self) -> LeafSize<Self::Leaf>;
    fn as_down(&self) -> Down<Self::Leaf>;
    fn from_down(down: Down<Self::Leaf>) -> Option<Self>;
    fn key(&self) -> Option<Key<Self::Leaf>>;
    fn next_sibling(&self) -> Option<Self> {
        self.next().and_then(|n| n.into_sibling())
    }
}

#[derive(Clone)]
pub enum Next<N: NodeRef> {
    Sibling(N),
    Parent(InternalNodeRef<N::Leaf>),
}

impl<N: NodeRef> Next<N> {
    pub fn into_sibling(self) -> Option<N> {
        match self {
            Self::Sibling(node) => Some(node),
            _ => None,
        }
    }

    pub fn into_parent(self) -> Option<InternalNodeRef<N::Leaf>> {
        match self {
            Self::Parent(node) => Some(node),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub enum Down<L: LeafRef> {
    Leaf(L),
    Internal(InternalNodeRef<L>),
}

impl<L: LeafRef> Down<L> {
    pub fn size(&self) -> LeafSize<L> {
        match self {
            Self::Leaf(node) => node.size(),
            Self::Internal(node) => node.size(),
        }
    }

    pub fn key(&self) -> Option<Key<L>> {
        match self {
            Self::Leaf(node) => node.key(),
            Self::Internal(node) => node.key(),
        }
    }

    pub fn into_node<N: NodeRef<Leaf = L>>(self) -> Option<N> {
        N::from_down(self)
    }
}

impl<'a, L: LeafRef> TryFrom<&'a Down<L>> for &'a InternalNodeRef<L> {
    type Error = ();

    fn try_from(down: &'a Down<L>) -> Result<Self, ()> {
        match down {
            Down::Leaf(_) => Err(()),
            Down::Internal(node) => Ok(node),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum NodeKind {
    Internal = 0,
    Leaf = 1,
}

impl NodeKind {
    pub const VARIANTS: [Self; 2] = [Self::Internal, Self::Leaf];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum NextKind {
    Sibling = 0,
    Parent = 1,
}

impl NextKind {
    pub const VARIANTS: [Self; 2] = [Self::Sibling, Self::Parent];
}

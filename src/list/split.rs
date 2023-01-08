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

use super::min_node_length;
use super::node::{InternalNodeRef, Next, NodeRef};
use crate::allocator::Allocator;
use crate::options::LeafSize;
use crate::PersistentAlloc;
use core::iter::FusedIterator;

pub struct Split<N: NodeRef> {
    node: Option<N>,
    /// Length of each chunk emitted by this iterator.
    chunk_len: usize,
    /// The first `extra` chunks will actually be 1 larger than `chunk_len`.
    extra: usize,
}

/// Data needed to create or initialize a new internal node.
pub struct InternalNodeSetup<N: NodeRef> {
    /// First child.
    start: N,
    /// Last child.
    end: N,
    /// Number of children.
    len: usize,
    /// Sum of child sizes.
    size: LeafSize<N::Leaf>,
}

impl<N: NodeRef> InternalNodeSetup<N> {
    pub fn apply_to(self, node: InternalNodeRef<N::Leaf>) {
        node.len.set(self.len);
        node.size.set(self.size);
        node.set_down(Some(self.start.as_down()));
        node.key.set(self.start.key());
        self.end.set_next(Some(Next::Parent(node)));
    }

    pub fn into_new<A>(
        self,
        alloc: &PersistentAlloc<A>,
    ) -> InternalNodeRef<N::Leaf>
    where
        A: Allocator,
    {
        let node = InternalNodeRef::alloc(alloc);
        self.apply_to(node);
        node
    }
}

impl<N: NodeRef> Iterator for Split<N> {
    type Item = InternalNodeSetup<N>;

    fn next(&mut self) -> Option<Self::Item> {
        let len = self.chunk_len + (self.extra > 0) as usize;
        self.extra = self.extra.saturating_sub(1);
        let start = self.node.take()?;
        let mut node = start.clone();
        let mut size = node.size();

        for _ in 1..len {
            node = node.next_sibling().unwrap();
            size += node.size();
        }

        self.node = node.next_sibling();
        Some(InternalNodeSetup {
            start,
            end: node,
            len,
            size,
        })
    }
}

impl<N: NodeRef> FusedIterator for Split<N> {}

/// Splits the sequence of `len` nodes starting at `N` into chunks with lengths
/// between the minimum and maximum (usually close to the minimum).
pub fn split<N: NodeRef>(node: N, len: usize) -> Split<N> {
    // Subtract 1 here so that we don't end up emitting two minimum-length
    // chunks instead of one maximum-length chunk if, e.g., `len` is equal
    // to the max chunk length.
    let num_chunks = 1.max((len - 1) / min_node_length::<N::Leaf>());
    Split {
        node: Some(node),
        chunk_len: len / num_chunks,
        extra: len % num_chunks,
    }
}

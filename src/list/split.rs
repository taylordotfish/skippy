use super::min_node_length;
use super::node::{InternalNodeRef, LeafRef, Next, NodeRef};
use crate::Allocator;
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
    size: <N::Leaf as LeafRef>::Size,
}

impl<N: NodeRef> InternalNodeSetup<N> {
    pub fn apply_to(self, node: InternalNodeRef<N::Leaf>) {
        node.len.set(self.len);
        node.size.set(self.size);
        node.set_down(Some(self.start.as_down()));
        node.key.set(self.start.key());
        self.end.set_next(Some(Next::Parent(node)));
    }

    pub fn into_new<A>(self, alloc: &A) -> InternalNodeRef<N::Leaf>
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

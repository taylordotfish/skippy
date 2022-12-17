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

use super::node::{InternalNodeRef, Next, NodeRef};

pub fn get_parent<N: NodeRef>(node: N) -> Option<InternalNodeRef<N::Leaf>> {
    get_parent_info(node).parent
}

pub struct ParentInfo<N: NodeRef> {
    pub parent: Option<InternalNodeRef<N::Leaf>>,
    pub last: N,
    pub index: usize,
}

pub fn get_parent_info<N: NodeRef>(node: N) -> ParentInfo<N> {
    let mut count = 0;
    let mut next = match node.next() {
        Some(next) => next,
        None => {
            return ParentInfo {
                parent: None,
                last: node,
                index: 0,
            };
        }
    };

    let mut last = node;
    let parent = loop {
        count += 1;
        next = match next {
            Next::Parent(node) => break node,
            Next::Sibling(node) => {
                let next = node.next().unwrap();
                last = node;
                next
            }
        };
    };
    ParentInfo {
        parent: Some(parent),
        last,
        index: parent.len.get() - count,
    }
}

pub fn get_nth_sibling<N: NodeRef>(mut node: N, n: usize) -> Option<N> {
    for _ in 0..n {
        node = node.next()?.into_sibling()?;
    }
    Some(node)
}

pub fn get_last_sibling<N: NodeRef>(node: N) -> N {
    get_parent_info(node).last
}

pub fn get_previous<N: NodeRef>(node: N) -> Option<Next<N>> {
    get_previous_info(node).previous.map(|p| p.node)
}

pub struct Previous<N: NodeRef> {
    pub node: Next<N>,
    pub parent: InternalNodeRef<N::Leaf>,
}

pub struct PreviousInfo<N: NodeRef> {
    pub last: N,
    pub index: usize,
    pub previous: Option<Previous<N>>,
}

pub fn get_previous_info<N: NodeRef>(node: N) -> PreviousInfo<N> {
    let ParentInfo {
        parent,
        last,
        index,
    } = get_parent_info(node);

    let parent = if let Some(parent) = parent {
        parent
    } else {
        return PreviousInfo {
            last,
            index,
            previous: None,
        };
    };

    let previous = if index == 0 {
        Next::Parent(parent)
    } else {
        let mut node: N = parent.down_as().unwrap();
        for _ in 1..index {
            node = node.next_sibling().unwrap();
        }
        Next::Sibling(node)
    };

    PreviousInfo {
        last,
        index,
        previous: Some(Previous {
            node: previous,
            parent,
        }),
    }
}

impl<N: NodeRef> From<PreviousInfo<N>> for ParentInfo<N> {
    fn from(info: PreviousInfo<N>) -> Self {
        Self {
            parent: info.previous.map(|p| p.parent),
            last: info.last,
            index: info.index,
        }
    }
}

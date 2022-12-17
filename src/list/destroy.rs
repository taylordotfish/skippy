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

use super::destroy_safety::can_safely_destroy;
use super::node::{Down, InternalNodeRef, LeafExt, LeafRef, Next, NodeRef};
use crate::allocator::Allocator;

/// Returns a node list that can be passed to [`destroy_node_list`].
pub fn deconstruct<L: LeafRef>(root: Down<L>) -> Option<InternalNodeRef<L>> {
    deconstruct_impl(root, None)
}

fn deconstruct_impl<L: LeafRef>(
    root: Down<L>,
    mut head: Option<InternalNodeRef<L>>,
) -> Option<InternalNodeRef<L>> {
    match root {
        Down::Leaf(mut node) => loop {
            let next = node.next_sibling();
            node.set_next_leaf(None);
            node = if let Some(next) = next {
                next
            } else {
                break;
            }
        },
        Down::Internal(mut node) => loop {
            if let Some(down) = node.down() {
                head = deconstruct_impl(down, head);
            }
            let next = node.next_sibling();
            node.set_next(head.map(Next::Sibling));
            head = Some(node);
            node = if let Some(next) = next {
                next
            } else {
                break;
            }
        },
    }
    head
}

/// # Safety
///
/// * Every node in the list must have been allocafed by `alloc`.
/// * There must be no other [`InternalNodeRef`]s that refer to any nodes in
///   the list.
pub unsafe fn destroy_node_list<L: LeafRef, A: Allocator>(
    head: &mut Option<InternalNodeRef<L>>,
    alloc: &A,
) {
    if !can_safely_destroy() {
        return;
    }
    while let Some(node) = head {
        let next = node.next_sibling();
        // SAFETY: Checked by caller.
        unsafe {
            node.dealloc(alloc);
        }
        *head = next;
    }
}

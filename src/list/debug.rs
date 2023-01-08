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

use super::node::{Down, InternalNodeRef, LeafRef, Next, NodeRef};
use super::SkipList;
use crate::allocator::Allocator;
use crate::options::LeafSize;
use alloc::collections::BTreeMap;
use core::cell::RefCell;
use core::fmt::{self, Debug, Display, Formatter};

// Indents for use in format strings
const I1: &str = "    ";
const I2: &str = "        ";

struct IdMap<T>(BTreeMap<T, usize>);

impl<T> IdMap<T> {
    pub fn new() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T: Ord> IdMap<T> {
    pub fn get(&mut self, value: T) -> usize {
        let len = self.0.len();
        *self.0.entry(value).or_insert(len + 1)
    }
}

pub trait LeafDebug: LeafRef {
    type Id: Ord;
    fn id(&self) -> Self::Id;
    fn fmt_data(&self, f: &mut Formatter<'_>) -> fmt::Result;
}

pub struct State<L: LeafDebug> {
    internal_map: IdMap<usize>,
    leaf_map: IdMap<L::Id>,
}

impl<L: LeafDebug> State<L> {
    pub fn new() -> Self {
        Self {
            internal_map: IdMap::new(),
            leaf_map: IdMap::new(),
        }
    }

    fn internal_id(&mut self, node: InternalNodeRef<L>) -> usize {
        self.internal_map.get(node.as_ptr().as_ptr() as _)
    }

    fn leaf_id(&mut self, node: &L) -> usize {
        self.leaf_map.get(node.id())
    }
}

impl<L: LeafDebug> Default for State<L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L, A> SkipList<L, A>
where
    L: LeafDebug,
    A: Allocator,
    LeafSize<L>: Debug,
{
    pub fn debug<'a>(
        &'a self,
        state: &'a mut State<L>,
    ) -> ListDebug<'a, L, A> {
        ListDebug {
            state: RefCell::new(state),
            list: self,
        }
    }
}

#[must_use]
pub struct ListDebug<'a, L, A>
where
    L: LeafDebug,
    A: Allocator,
{
    state: RefCell<&'a mut State<L>>,
    list: &'a SkipList<L, A>,
}

impl<'a, L, A> Display for ListDebug<'a, L, A>
where
    L: LeafDebug,
    A: Allocator,
    LeafSize<L>: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut state = self.state.borrow_mut();
        writeln!(f, "digraph {{")?;
        fmt_down(*state, f, self.list.root.clone())?;
        writeln!(f, "}}")
    }
}

fn fmt_down<L>(
    state: &mut State<L>,
    f: &mut Formatter<'_>,
    node: Option<Down<L>>,
) -> fmt::Result
where
    L: LeafDebug,
    LeafSize<L>: Debug,
{
    match node {
        Some(Down::Internal(node)) => fmt_internal(state, f, node),
        Some(Down::Leaf(node)) => fmt_leaf(state, f, node),
        None => Ok(()),
    }
}

fn fmt_internal<L>(
    state: &mut State<L>,
    f: &mut Formatter<'_>,
    node: InternalNodeRef<L>,
) -> fmt::Result
where
    L: LeafDebug,
    LeafSize<L>: Debug,
{
    let mut n = node;
    writeln!(f, "{I1}{{\n{I2}rank=same")?;
    loop {
        let id = state.internal_id(n);
        writeln!(
            f,
            "{I2}i{id} [label=\"i{id}\\nL: {}\\nS: {:?}\" shape=rectangle]",
            n.len.get(),
            n.size(),
        )?;
        if let Some(next) = n.next_sibling() {
            n = next;
        } else {
            break;
        }
    }
    writeln!(f, "{I1}}}")?;

    n = node;
    loop {
        let id = state.internal_id(n);
        match n.down() {
            Some(Down::Internal(down)) => {
                writeln!(f, "{I1}i{id} -> i{}", state.internal_id(down))?;
            }
            Some(Down::Leaf(down)) => {
                writeln!(f, "{I1}i{id} -> L{}", state.leaf_id(&down))?;
            }
            None => {}
        }
        fmt_down(state, f, n.down())?;
        match NodeRef::next(&n) {
            Some(Next::Sibling(next)) => {
                writeln!(
                    f,
                    "{I1}i{id} -> i{} [arrowhead=onormal]",
                    state.internal_id(next),
                )?;
                n = next;
            }
            Some(Next::Parent(next)) => {
                writeln!(
                    f,
                    "{I1}i{id} -> i{} [style=dashed arrowhead=onormal]",
                    state.internal_id(next),
                )?;
                break;
            }
            None => break,
        }
    }
    Ok(())
}

pub fn fmt_leaf<L>(
    state: &mut State<L>,
    f: &mut Formatter<'_>,
    node: L,
) -> fmt::Result
where
    L: LeafDebug,
    LeafSize<L>: Debug,
{
    let mut n = node.clone();
    writeln!(f, "{I1}{{\n{I2}rank=same")?;
    loop {
        let id = state.leaf_id(&n);
        write!(f, "{I2}L{id} [label=\"L{id}\\n")?;
        n.fmt_data(f)?;
        writeln!(f, "\" shape=rectangle]")?;
        if let Some(next) = n.next_sibling() {
            n = next;
        } else {
            break;
        }
    }
    writeln!(f, "{I1}}}")?;

    n = node;
    loop {
        let id = state.leaf_id(&n);
        match NodeRef::next(&n) {
            Some(Next::Sibling(next)) => {
                writeln!(
                    f,
                    "{I1}L{id} -> L{} [arrowhead=onormal]",
                    state.leaf_id(&next),
                )?;
                n = next;
            }
            Some(Next::Parent(next)) => {
                writeln!(
                    f,
                    "{I1}L{id} -> i{} [style=dashed arrowhead=onormal]",
                    state.internal_id(next),
                )?;
                break;
            }
            None => break,
        }
    }
    Ok(())
}

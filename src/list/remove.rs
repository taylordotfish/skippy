use super::min_node_length;
use super::node::{Down, InternalNodeRef, LeafRef, Next, NodeRef};
use super::traverse::{get_nth_sibling, get_previous, get_previous_info};
use cell_mut::CellExt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemovalKind {
    Remove, // remove the node
    Update, // just propagate changes from a downstream removal
}

struct Removal<N: NodeRef> {
    pub child: N,
    pub kind: RemovalKind,
    pub diff: <N::Leaf as LeafRef>::Size,
}

impl<N: NodeRef> Removal<N> {
    pub fn remove(child: N, diff: <N::Leaf as LeafRef>::Size) -> Self {
        Self {
            kind: RemovalKind::Remove,
            child,
            diff,
        }
    }

    pub fn update(child: N, diff: <N::Leaf as LeafRef>::Size) -> Self {
        Self {
            kind: RemovalKind::Update,
            child,
            diff,
        }
    }
}

enum RemovalResult<N: NodeRef> {
    Removal(Removal<InternalNodeRef<N::Leaf>>),
    Done(N),
}

pub struct FinishedRemoval<L: LeafRef> {
    pub old_root: Down<L>,
    pub new_root: Option<Down<L>>,
    pub removed: Option<InternalNodeRef<L>>,
}

fn handle_removal<N: NodeRef>(removal: Removal<N>) -> RemovalResult<N> {
    let child = removal.child;
    let diff = removal.diff;
    let info = get_previous_info(child.clone());
    let (parent, previous) = if let Some(prev) = info.previous {
        (prev.parent, prev.node)
    } else {
        return RemovalResult::Done(child);
    };

    parent.size.with_mut(|s| *s -= diff.clone());
    if removal.kind == RemovalKind::Update {
        return RemovalResult::Removal(Removal::update(parent, diff));
    }

    match &previous {
        Next::Sibling(node) => node.set_next(child.next()),
        Next::Parent(node) => {
            node.set_down(Some(child.next_sibling().unwrap().as_down()))
        }
    };

    let first: N = parent.down_as().unwrap();
    let last = if info.position + 1 == parent.len.get() {
        previous.into_sibling().unwrap()
    } else {
        info.last
    };

    child.set_next(None);
    parent.len.with_mut(|n| *n -= 1);
    if parent.len.get() >= min_node_length::<N::Leaf>() {
        return RemovalResult::Removal(Removal::update(parent, diff));
    }

    let (neighbor, is_right) = match parent.next() {
        None => return RemovalResult::Removal(Removal::update(parent, diff)),
        Some(Next::Sibling(right)) => (right, true),
        Some(Next::Parent(_)) => {
            (get_previous(parent).unwrap().into_sibling().unwrap(), false)
        }
    };

    if is_right {
        let right = neighbor;
        let right_first: N = right.down_as().unwrap();
        if right.len.get() > min_node_length::<N::Leaf>() {
            let right_second = right_first.next_sibling().unwrap();
            right.len.with_mut(|n| *n -= 1);
            parent.len.with_mut(|n| *n += 1);
            right.size.with_mut(|s| *s -= right_first.size());
            parent.size.with_mut(|s| *s += right_first.size());

            right.set_down(Some(right_second.as_down()));
            right_first.set_next(last.next());
            right.key.set(right_second.key().into());
            last.set_next(Some(Next::Sibling(right_first)));
            return RemovalResult::Removal(Removal::update(parent, diff));
        }

        right.set_down(Some(first.as_down()));
        last.set_next(Some(Next::Sibling(right_first)));
        parent.set_down(None);
        right.size.with_mut(|s| *s += parent.size.take());
        right.len.with_mut(|n| *n += parent.len.take());
        return RemovalResult::Removal(Removal::remove(parent, diff));
    }

    let left = neighbor;
    let left_len = left.len.get();
    let left_first: N = left.down_as().unwrap();
    let left_penultimate =
        get_nth_sibling(left_first.clone(), left_len - 2).unwrap();
    let left_last = left_penultimate.next_sibling().unwrap();

    if left_len > min_node_length::<N::Leaf>() {
        left.len.with_mut(|n| *n -= 1);
        parent.len.with_mut(|n| *n += 1);
        left.size.with_mut(|s| *s -= left_last.size());
        parent.size.with_mut(|s| *s += left_last.size());

        left_penultimate.set_next(left_last.next());
        left_last.set_next(Some(Next::Sibling(first)));
        parent.set_down(Some(left_last.as_down()));
        parent.key.set(left_last.key().into());
        return RemovalResult::Removal(Removal::update(parent, diff));
    }

    parent.set_down(Some(left_first.as_down()));
    left_last.set_next(Some(Next::Sibling(first)));
    left.set_down(None);
    parent.size.with_mut(|s| *s += left.size.take());
    parent.len.with_mut(|n| *n += left.len.take());
    RemovalResult::Removal(Removal::remove(parent, diff))
}

pub fn remove<L: LeafRef>(item: L) -> FinishedRemoval<L> {
    let size = item.size();
    let result = handle_removal(Removal::remove(item, size));
    let mut head = None;
    let mut removal = match result {
        RemovalResult::Removal(removal) => removal,
        RemovalResult::Done(root) => {
            return FinishedRemoval {
                old_root: root.as_down(),
                new_root: None,
                removed: head,
            };
        }
    };

    let root = loop {
        let child = match removal.kind {
            RemovalKind::Remove => Some(removal.child),
            RemovalKind::Update => None,
        };
        let result = handle_removal(removal);
        if let Some(child) = child {
            child.set_next(head.map(Next::Sibling));
            head = Some(child);
        }
        removal = match result {
            RemovalResult::Removal(removal) => removal,
            RemovalResult::Done(root) => break root,
        };
    };

    let new_root = if root.len.get() <= 1 {
        let down = root.down().unwrap();
        match &down {
            Down::Leaf(node) => node.set_next(None),
            Down::Internal(node) => node.set_next(None),
        };
        root.set_next(head.map(Next::Sibling));
        head = Some(root);
        down
    } else {
        root.as_down()
    };

    FinishedRemoval {
        old_root: root.as_down(),
        new_root: Some(new_root),
        removed: head,
    }
}

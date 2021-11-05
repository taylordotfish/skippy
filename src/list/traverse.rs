use super::node::{InternalNodeRef, Next, NodeRef};

pub fn get_parent<N: NodeRef>(node: N) -> Option<InternalNodeRef<N::Leaf>> {
    get_parent_info(node).parent
}

pub struct ParentInfo<N: NodeRef> {
    pub parent: Option<InternalNodeRef<N::Leaf>>,
    pub last: N,
    pub position: usize,
}

pub fn get_parent_info<N: NodeRef>(node: N) -> ParentInfo<N> {
    let mut count = 0;
    let mut next = match node.next() {
        Some(next) => next,
        None => {
            return ParentInfo {
                parent: None,
                last: node,
                position: 0,
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
        position: parent.len.get() - count,
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
    pub position: usize,
    pub previous: Option<Previous<N>>,
}

pub fn get_previous_info<N: NodeRef>(node: N) -> PreviousInfo<N> {
    let ParentInfo {
        parent,
        last,
        position,
    } = get_parent_info(node);

    let parent = if let Some(parent) = parent {
        parent
    } else {
        return PreviousInfo {
            last,
            position,
            previous: None,
        };
    };

    let previous = if position == 0 {
        Next::Parent(parent)
    } else {
        let mut node: N = parent.down_as().unwrap();
        for _ in 1..position {
            node = node.next_sibling().unwrap();
        }
        Next::Sibling(node)
    };

    PreviousInfo {
        last,
        position,
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
            position: info.position,
        }
    }
}

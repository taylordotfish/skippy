use super::{basic::*, *};
use core::cell::Cell;
use core::fmt;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Data(usize, Cell<usize>);

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.0, self.1.get())
    }
}

impl Data {
    pub fn new(n: usize, size: usize) -> Self {
        Self(n, Cell::new(size))
    }
}

impl BasicLeaf for Data {
    const FANOUT: usize = 4;
    type Size = usize;
    type StoreKeys = StoreKeys<true>;

    fn size(&self) -> Self::Size {
        self.1.get()
    }
}

#[test]
fn basic() {
    let items: Vec<_> =
        (0..250).map(|n| RefLeaf::new(Data::new(n, 1))).collect();

    let mut list = SkipList::new();
    list.push_front_from(items.iter());

    assert_eq!(list.size(), items.len());
    assert!(list.iter().eq(&items));

    for i in 0..items.len() {
        assert_eq!(i, list.get(&i).unwrap().0);
        assert_eq!(i, list.find_with(&i, |r| r.0).ok().unwrap().0);
        assert_eq!(
            i,
            list.find_with(&(i * 2 + 1), |r| r.0 * 2)
                .err()
                .unwrap()
                .unwrap()
                .0,
        );
    }

    assert!(list.get(&items.len()).is_none());
    assert!(list.find_with(&0, |r| r.0 + 1).is_err());
    assert!(list.find_with(&items.len(), |r| r.0).is_err());
}

#[test]
fn push_back() {
    let items: Vec<_> =
        (0..150).map(|n| RefLeaf::new(Data::new(n, 1))).collect();

    let mut list = SkipList::new();
    for item in items.iter() {
        list.push_back(item);
    }
    assert!(list.iter().eq(&items));
}

#[test]
fn push_front() {
    let items: Vec<_> =
        (0..200).map(|n| RefLeaf::new(Data::new(n, 1))).collect();

    let mut list = SkipList::new();
    for item in items.iter().rev() {
        list.push_front(item);
    }
    assert!(list.iter().eq(&items));
}

#[test]
fn insert() {
    let items: Vec<_> =
        (0..250).map(|n| RefLeaf::new(Data::new(n, 1))).collect();

    let mut refs = Vec::with_capacity(items.len());
    let mut list = SkipList::new();

    for (pos, range, before) in [
        (0, 0..50, false),
        (25, 50..60, false),
        (5, 60..80, false),
        (78, 80..81, true),
        (40, 81..82, false),
        (15, 82..126, true),
        (100, 126..146, true),
        (90, 146..186, false),
        (186, 186..226, false),
        (0, 226..250, false),
    ] {
        if before {
            list.insert_before_from(refs[pos], &items[range.clone()]);
        } else if pos > 0 {
            list.insert_after_from(refs[pos - 1], &items[range.clone()]);
        } else {
            list.insert_after_opt_from(None, &items[range.clone()]);
        }
        refs.splice(pos..pos, &items[range]);
    }
    assert!(list.iter().eq(refs.iter().copied()));
}

#[test]
fn remove() {
    let items: Vec<_> =
        (0..250).map(|n| RefLeaf::new(Data::new(n, 1))).collect();

    let mut refs = Vec::from_iter(&items);
    let mut list = SkipList::new();
    list.push_back_from(&items);

    [20; 10]
        .into_iter()
        .chain([0; 10])
        .chain([100, 120])
        .chain([50; 30])
        .chain([83, 101, 25, 3, 16])
        .chain([80; 20])
        .for_each(|i| {
            list.remove(refs[i]);
            refs.remove(i);
        });
    assert!(list.iter().eq(refs.iter().copied()));
}

#[cfg(skip_list_debug)]
#[allow(dead_code)]
fn make_graph<L>(
    list: &SkipList<L>,
    state: &mut debug::State<L>,
) -> std::io::Result<()>
where
    L: debug::LeafDebug,
    L::Size: fmt::Debug,
{
    use std::fs::File;
    use std::io::Write;
    use std::process::Command;

    let mut file = File::create("graph.dot")?;
    write!(file, "{}", list.debug(state))?;
    file.sync_all()?;
    drop(file);
    Command::new("dot")
        .arg("-Tpng")
        .arg("-ograph.png")
        .arg("graph.dot")
        .status()?;
    Ok(())
}

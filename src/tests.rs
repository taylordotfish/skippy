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

use crate::SkipList;
use crate::basic::{self, BasicLeaf, RefLeaf};
use alloc::vec::Vec;
use core::cell::Cell;
use core::cmp::Ordering;
use core::fmt;

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Data {
    value: usize,
    size: Cell<usize>,
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {})", self.value, self.size.get())
    }
}

impl Data {
    pub fn new(n: usize, size: usize) -> Self {
        Self {
            value: n,
            size: Cell::new(size),
        }
    }
}

impl BasicLeaf for Data {
    type Options = basic::options::Options<
        /* SizeType */ usize,
        /* STORE_KEYS */ true,
        /* FANOUT */ 4,
    >;

    fn size(&self) -> usize {
        self.size.get()
    }
}

type Leaf<'a> = RefLeaf<'a, Data>;

struct Value<F> {
    value: usize,
    transformation: F,
}

impl Value<()> {
    pub fn new(value: usize) -> Value<impl Fn(usize) -> usize> {
        Value {
            value,
            transformation: |v| v,
        }
    }
}

impl<F> Value<F> {
    pub fn with_transformation(value: usize, tf: F) -> Self {
        Self {
            value,
            transformation: tf,
        }
    }
}

impl<F: Fn(usize) -> usize> PartialEq<&Leaf<'_>> for Value<F> {
    fn eq(&self, other: &&Leaf<'_>) -> bool {
        self.partial_cmp(other) == Some(Ordering::Equal)
    }
}

impl<F: Fn(usize) -> usize> PartialEq<Value<F>> for &Leaf<'_> {
    fn eq(&self, other: &Value<F>) -> bool {
        other == self
    }
}

impl<F: Fn(usize) -> usize> PartialOrd<&Leaf<'_>> for Value<F> {
    fn partial_cmp(&self, other: &&Leaf<'_>) -> Option<Ordering> {
        Some(self.value.cmp(&(self.transformation)(other.value)))
    }
}

impl<F: Fn(usize) -> usize> PartialOrd<Value<F>> for &Leaf<'_> {
    fn partial_cmp(&self, other: &Value<F>) -> Option<Ordering> {
        other.partial_cmp(self).map(Ordering::reverse)
    }
}

#[test]
fn basic() {
    let items: Vec<_> = (0..250).map(|n| Leaf::new(Data::new(n, 1))).collect();
    let mut list = SkipList::new();
    list.push_front_from(items.iter());

    assert_eq!(list.size(), items.len());
    assert!(list.iter().eq(&items));

    for i in 0..items.len() {
        let item = list.get(&i).unwrap();
        assert_eq!(i, item.value);
        assert_eq!(SkipList::index(item), i);
        assert_eq!(i, list.find_with(&Value::new(i)).ok().unwrap().value);
        let v = Value::with_transformation(i * 2 + 1, |v| v * 2);
        assert_eq!(i, list.find_with(&v).err().unwrap().unwrap().value);
    }

    assert_eq!(list.get(&items.len()), None);
    assert!(
        list.find_with(&Value::with_transformation(0, |v| v + 1)).is_err(),
    );
    assert!(list.find_with(&Value::new(items.len())).is_err());
}

#[test]
fn push_back() {
    let items: Vec<_> = (0..150).map(|n| Leaf::new(Data::new(n, 1))).collect();
    let mut list = SkipList::new();
    for item in items.iter() {
        list.push_back(item);
    }
    assert!(list.iter().eq(&items));
}

#[test]
fn push_front() {
    let items: Vec<_> = (0..200).map(|n| Leaf::new(Data::new(n, 1))).collect();
    let mut list = SkipList::new();
    for item in items.iter().rev() {
        list.push_front(item);
    }
    assert!(list.iter().eq(&items));
}

#[test]
fn insert() {
    let items: Vec<_> = (0..250).map(|n| Leaf::new(Data::new(n, 1))).collect();
    let mut refs = Vec::with_capacity(items.len());
    let mut list = SkipList::new();

    for (index, range, before) in [
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
            list.insert_before_from(refs[index], &items[range.clone()]);
        } else {
            let pos = index.checked_sub(1).map(|i| refs[i]);
            list.insert_after_opt_from(pos, &items[range.clone()]);
        }
        refs.splice(index..index, &items[range]);
    }
    assert!(list.iter().eq(refs.iter().copied()));
}

#[test]
fn remove() {
    let items: Vec<_> = (0..250).map(|n| Leaf::new(Data::new(n, 1))).collect();
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

#[test]
fn get_after() {
    let items: Vec<_> = (0..250).map(|n| Leaf::new(Data::new(n, 1))).collect();
    let mut list = SkipList::new();
    list.push_back_from(&items);
    let item = list.get(&100).unwrap();
    assert_eq!(item.value, 100);
    assert_eq!(SkipList::get_after(item, &0).unwrap().value, 100);
    assert_eq!(SkipList::get_after(item, &1).unwrap().value, 101);
    assert_eq!(SkipList::get_after(item, &2).unwrap().value, 102);
    assert_eq!(SkipList::get_after(item, &10).unwrap().value, 110);
    assert_eq!(SkipList::get_after(item, &20).unwrap().value, 120);
    assert_eq!(SkipList::get_after(item, &50).unwrap().value, 150);
    assert_eq!(SkipList::get_after(item, &100).unwrap().value, 200);
    assert_eq!(SkipList::get_after(item, &149).unwrap().value, 249);
    assert_eq!(SkipList::get_after(item, &150), None);
    let item = list.last().unwrap();
    assert_eq!(SkipList::get_after(item, &0), Some(item));
    assert_eq!(SkipList::get_after(item, &1), None);
}

#[test]
fn find_after() {
    let items: Vec<_> = (0..250).map(|n| Leaf::new(Data::new(n, 1))).collect();
    let mut list = SkipList::new();
    list.push_back_from(&items);
    let item = list.find_with(&Value::new(50)).unwrap();
    assert_eq!(item.value, 50);
    let find = |v| SkipList::find_after_with(item, &Value::new(v));
    assert_eq!(find(0), Err(None));
    assert_eq!(find(49), Err(None));
    assert_eq!(find(50).unwrap().value, 50);
    assert_eq!(find(51).unwrap().value, 51);
    assert_eq!(find(100).unwrap().value, 100);
    assert_eq!(find(200).unwrap().value, 200);
    assert_eq!(find(249).unwrap().value, 249);
    assert_eq!(find(250).unwrap_err().unwrap().value, 249);
}

#[test]
fn zero_sized() {
    let mut items = Vec::new();
    for i in 0..101 {
        items.push(Leaf::new(Data::new(i, i % 2)));
    }
    let mut list = SkipList::new();
    list.push_back_from(&items);
    assert_eq!(list.size(), 50);
    assert_eq!(list.get(&0).unwrap().value, 1);
    assert_eq!(list.get(&1).unwrap().value, 3);
    assert_eq!(list.get(&10).unwrap().value, 21);
    assert_eq!(list.get(&25).unwrap().value, 51);
    assert_eq!(list.get(&42).unwrap().value, 85);
    assert_eq!(list.get(&49).unwrap().value, 99);
    assert_eq!(list.get(&50).unwrap().value, 100);
    let item = list.get(&25).unwrap();
    assert_eq!(item.value, 51);
    assert_eq!(SkipList::get_after(item, &0).unwrap().value, 51);
    assert_eq!(SkipList::get_after(item, &15).unwrap().value, 81);
    let item = SkipList::next(item).unwrap();
    assert_eq!(item.value, 52);
    assert_eq!(SkipList::get_after(item, &0).unwrap().value, 53);
    assert_eq!(SkipList::get_after(item, &15).unwrap().value, 83);
    assert_eq!(SkipList::get_after(item, &23).unwrap().value, 99);
    assert_eq!(SkipList::get_after(item, &24).unwrap().value, 100);
    let item = list.get(&49).unwrap();
    assert_eq!(SkipList::get_after(item, &0).unwrap().value, 99);
    assert_eq!(SkipList::get_after(item, &1).unwrap().value, 100);
    let item = list.get(&50).unwrap();
    assert_eq!(item.value, 100);
    assert_eq!(SkipList::get_after(item, &0).unwrap().value, 100);
    assert_eq!(SkipList::get_after(item, &1), None);
}

#[test]
fn one_item() {
    use core::ptr::addr_eq;
    let item = Leaf::new(Data::new(123, 1));
    let mut list = SkipList::new();
    list.push_front(&item);
    assert!(addr_eq(list.first().unwrap(), &item));
    assert!(addr_eq(list.last().unwrap(), &item));
    assert_eq!(SkipList::index(&item), 0);
    assert_eq!(SkipList::next(&item), None);
}

#[test]
fn large_items() {
    let items: Vec<_> = (0..30).map(|n| Leaf::new(Data::new(n, 10))).collect();
    let mut list = SkipList::new();
    list.push_front_from(&items);
    assert_eq!(list.get(&5).unwrap().value, 0);
    assert_eq!(list.get(&9).unwrap().value, 0);
    assert_eq!(list.get(&10).unwrap().value, 1);
    assert_eq!(list.get(&99).unwrap().value, 9);
    assert_eq!(list.get(&100).unwrap().value, 10);
    assert_eq!(list.get(&299).unwrap().value, 29);
    assert_eq!(list.get(&300), None);
}

#[cfg(skippy_debug)]
#[allow(dead_code)]
fn make_graph<L>(
    list: &SkipList<L>,
    state: &mut crate::debug::State<L>,
) -> std::io::Result<()>
where
    L: crate::debug::LeafDebug,
    crate::options::LeafSize<L>: fmt::Debug,
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

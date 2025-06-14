Skippy
======

A highly flexible, non-amortized worst-case O(log *n*) **intrusive** skip list.

The skip list can be used both as a sorted sequence (allowing it to be used as
a set or map) and as an unsorted sequence of elements in arbitrary order
(allowing it to be used as a vector/dynamic array). Elements support an
optional notion of “size”, allowing insertions, removals, and lookups by index,
as well as, due to the intrusive nature of the skip list, the ability to query
an element’s index.

Internal nodes in the skip list are allocated deterministically so that between
any two consecutive nodes at layer *L*, there are always between *F* / 2 and
*F* nodes at layer *L* - 1, where *F* is a configurable [fanout] parameter.
This is very similar to a B+ tree; in fact, this skip list *is* essentially a
B+ tree where children are stored in linked lists rather than arrays.

Crate features
--------------

If the crate feature `allocator_api` is enabled, the skip list can be
configured with the unstable [`Allocator`] trait. Otherwise,
[allocator-fallback] will be used.

This crate can be used in `no_std` contexts by disabling the `std` feature with
`default-features = false`. In this case, one of `allocator-fallback` or
`allocator_api` must be enabled.

[fanout]: https://doc.rust-lang.org/skippy/0.1/skippy/options/trait.ListOptions.html#associatedtype.Fanout
[`Allocator`]: https://doc.rust-lang.org/stable/std/alloc/trait.Allocator.html
[allocator-fallback]: https://docs.rs/allocator-fallback

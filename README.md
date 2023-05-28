Skippy
======

A highly flexible, non-amortized worst-case O(log *n*) intrusive skip list.

The skip list can be used both as an ordered sequence (allowing it to be
used like a set or map) and as an unordered sequence (allowing it to be
used like a vector/dynamic array). Elements support an optional notion of
“size”, allowing insertions, removals, and lookups by index, as well as,
due to the intrusive nature of the skip list, the ability to query an
element’s index.

Documentation
-------------

[Documentation is available on docs.rs.](https://docs.rs/skippy)

License
-------

Skippy is licensed under version 3 of the GNU Affero General Public License, or
(at your option) any later version. See [LICENSE](LICENSE).

Contributing
------------

By contributing to btree-vec, you agree that your contribution may be used
according to the terms of Skippy’s license.

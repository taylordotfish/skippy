Skippy
======

A highly flexible, non-amortized worst-case O(log n) intrusive skip list.

The skip list can be used both as an ordered sequence (allowing it to be
used like a set or map) and as an unordered sequence (allowing it to be
used like a vector/dynamic array). Elements support an optional notion of
“size”, allowing insertions, removals, and lookups by index, as well as,
due to the intrusive nature of the skip list, the ability to query an
element’s index.

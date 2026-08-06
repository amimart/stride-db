# stride-db

`stride-db` is an experimental persistent key-value store built around a
Strided Adaptive Radix Trie (SART), memory-mapped immutable storage, and
snapshot MVCC.

The intended read path is lock-free and allocation-free after a read
transaction has been opened. Keys and values are returned as borrowed slices
from immutable mappings when the on-disk representation permits it.

## Motivation

SART started from a future-work observation in the original
[Adaptive Radix Trees](https://db.in.tum.de/~leis/papers/ART.pdf) paper:

> Another idea is to design a space-efficient radix tree which has nodes of
> equal size. Instead of dynamically adapting the fanout based on the
> sparseness of the keys, the number of bits used from the key should change
> dynamically, while the fanout stays approximately constant. Such a tree
> could also be used for data stored on disk.

`stride-db` explores that idea as a persistent, mmap-oriented key-value store,
combining fixed-size routing nodes with adaptive key strides and ephemeral
MVCC snapshots. The paper's companion idea—latch-free synchronization using
atomic primitives—inspires a later exploration of optimistic root publication.

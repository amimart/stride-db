# stride-db design

## Status and scope

`stride-db` is a persistent ordered key-value store using a Strided Adaptive
Radix Trie (SART). Each inner node has at most 32 children and changes how many
key bits it consumes instead of changing among Node4/16/48/256
representations.

It does retain internal commit sequence numbers and transient old roots. Those
are MVCC implementation details, not historical database versions. A caller
can read a consistent snapshot only while its read transaction is alive; it
cannot later reopen snapshot 42.

The first implementation has one writer and many concurrent readers. This is
an intentional simplification: serialized commits make durability and
reclamation tractable, while readers remain lock-free after opening a read
transaction.

## Goals

- Arbitrary byte keys, including the empty key, in byte-lexicographic order.
- `Get`, `Contains`, insert/upsert, delete, and ordered range iteration.
- Snapshot-isolated read transactions and atomic multi-key write transactions.
- Allocation-free point reads after a read transaction is opened.
- Borrowed keys and values directly from read-only mappings when possible.
- Bounded 32-way SART routing nodes and adaptive strides from 1 to 16 bits.
- Crash-atomic commits and deterministic recovery after process or power loss.
- Reclamation bounded by active reader lifetimes rather than retained history.
- A validated, endian-stable disk format with checksums and explicit format
  evolution.

"Zero allocation" is a hot-path target, not a blanket claim. Opening a
transaction, growing the file, updating the writer overlay, and maintaining
the reader registry may allocate. Compression, encryption, or values spanning
non-contiguous extents may also require copying unless exposed as chunks.

## Non-goals for the first implementation

- Historical reads after a read transaction closes.
- Compatibility with another database format.
- Multiple processes writing the same database.
- Lock-free or concurrent writers.
- Serializable predicate transactions. The first write model is snapshot
  isolation with a serialized commit phase.
- In-place mutation of bytes visible through a read-only transaction.
- Transparent compression or encryption.
- Memory-mapping the entire growing file as one region.

## Key model and SART

Keys are MSB-first bit strings so bitwise trie order matches byte order. An
inner node consumes a compressed prefix followed by a stride digit:

```text
key[depth .. depth + prefix_bits] == prefix
digit = key[depth + prefix_bits .. + stride]
```

An optional terminal entry represents a key ending at the inner node. The
database accepts the empty key; it is represented by a terminal at the logical
root (or equivalent root metadata), not by a sentinel user key.

Each inner node contains:

- a prefix reference and bit range;
- a stride in `[1, 16]`;
- up to 32 sorted `(digit, child address)` entries;
- an optional terminal leaf address; and
- format flags/type information.

Each leaf contains references to immutable key and value bytes. Key bytes are
stored once. Inner prefixes refer to a stable range of a representative key
record rather than copying arbitrarily long prefixes into every fixed-size
node. Reclamation must consequently treat that key record as reachable from
the inner node.

Insertion handles prefix splits, short keys, 33rd-child reshapes, deletion
compaction, and lower-bound traversal directly in the SART structure. Nodes
contain only routing and storage metadata, with no historical-state fields.

### Fixed-size nodes

Inner nodes use one fixed encoded size with 32 physical child slots. It trades
some space and write bandwidth for:

- direct address calculation and validation;
- bounded decoding with no heap allocation;
- predictable copy-on-write cost;
- simple free-list size classes; and
- a representation that can be read in place.

The exact size and alignment are not frozen yet. They must be selected from
measurements of 512-byte, 1-KiB, and page-packed layouts. A likely child slot
is a 16-byte aligned pair containing a 16-bit digit, flags, and a 64-bit
address. Empty slots are zeroed and never part of logical ordering.

Leaves, keys, and values are variable-size records in aligned extents. Large
values may span chunks. The public API should prefer a contiguous borrowed
slice when possible and expose chunk iteration rather than silently allocate
for fragmented values.

## MVCC model: immutable roots, ephemeral history

The core MVCC unit is an immutable SART root.

Opening a read transaction atomically captures:

```text
(commit_sequence, root_address, mapping_generation)
```

All nodes, keys, and values reachable from that root are immutable. The read
transaction pins both the root generation and the mapping set, so returned
`&[u8]` values cannot outlive the bytes they reference.

A write transaction starts from one root and maintains a private overlay. On
its first change to a committed path it copies that path into newly allocated
records; further changes in the same transaction mutate or replace only its
unpublished records. At commit it writes the complete new reachable state and
publishes the new root once.

This is root-based MVCC rather than a version chain on every key:

- readers of the old root naturally see old leaves;
- readers of the new root naturally see new leaves;
- a leaf does not need `begin_ts`, `end_ts`, or a previous-value pointer;
- transactions cannot observe a partially installed multi-key commit; and
- repeated updates inside one write transaction coalesce before publication.

Old roots are not catalogued for callers. Replaced records are placed on a
retired list tagged with the commit sequence that replaced them. They become
reusable only when the oldest active reader is newer than that sequence. This
is epoch-based reclamation supporting MVCC, not persistent history.

### Isolation and conflicts

The initial engine permits one active writer. A write transaction can read its
own overlay and the root from which it began. Since commits are serialized, it
cannot lose a concurrent writer's update. Read-only transactions provide
repeatable reads.

If multiple write transactions are later allowed, commit must validate that
the base root is still current. The first acceptable policy is whole-root
optimistic conflict (retry any stale writer); key- or range-level validation
should be introduced only with strong benchmark evidence.

### Latch-free synchronization: a future direction

The other future-work idea from the ART paper is relevant, but "latch-free"
needs a precise scope here. Fixed-size SART nodes do not by themselves make a
durable database latch-free. File growth, flushing, metadata persistence, and
space reclamation can block; therefore the first useful goal is a **latch-free
read path and optimistic in-memory root publication**, not lock-free durable
commits.

The natural design builds on immutable copy-on-write paths:

1. A writer reads and epoch-pins the current immutable root descriptor.
2. It builds a private replacement path and any new key/value extents without
   changing the published tree.
3. It attempts a compare-and-swap of one root-descriptor pointer from its base
   descriptor to a new immutable descriptor.
4. A failed CAS means another writer won; the transaction rebases or reports a
   conflict. Its unpublished append-only extents are garbage collected later.
5. A successful publication retires the old root only after an epoch/hazard
   scheme proves no reader can still reach it.

Readers acquire an epoch pin before loading the root descriptor with acquire
ordering, then traverse immutable nodes without latches, retries, or shared
state updates. This is a stronger and simpler property than per-node CAS:
readers see either the entire old root or the entire new root, so a multi-key
transaction cannot become visible halfway through.

An atomic pointer to an immutable `RootDescriptor` is preferable to trying to
atomically update a `(root_address, sequence)` pair. The descriptor contains
both fields and is published with release ordering, avoiding a portability
dependency on double-width atomics. It must remain allocated until the reader
epoch has passed; reference counting alone is not enough for a zero-allocation
hot read path.

This leaves an important boundary: CAS can order logical in-memory publication,
but it does not safely serialize persistent metadata. Concurrent durable
commits still need a short commit sequencer (or an append-only commit log with
a single durable order). The sequencer is acceptable in an initial design:
writers may prepare in parallel, while only final durability ordering is
serialized. Calling that whole database "lock-free" would be misleading.

Per-node CAS is deliberately deferred. It would require update descriptors,
helping rules, and careful handling of SART reshapes that replace a bounded
node with several nodes. It also does not by itself make a multi-key
transaction atomic. Root CAS preserves the clean immutable format and is the
right experiment before considering finer-grained synchronization.

Long readers delay reuse. Options should eventually cap retired bytes and
either apply writer backpressure or reject new long-lived snapshots. The
engine must never invalidate a live borrowed slice to relieve pressure.

## mmap and storage architecture

### Files

The initial directory layout is:

```text
LOCK       exclusive writer/process lock
META       two fixed superblock slots
DATA       immutable node, key, value, and allocator records
```

Keeping metadata separate prevents a growing data mapping from disturbing the
atomic publication area. A separate write-ahead log is not required for the
first append-only commit protocol; adding one becomes useful when reclaimed
extents are overwritten in place.

All persisted integers have an explicitly selected endianness, all record
lengths are checked before slicing, and every independently recoverable record
has a type/version plus checksum. Process pointers, Rust enum layouts, and
`usize` are never persisted.

### Segmented mappings

`DATA` is divided into fixed-size segments (for example 64 MiB). Segments are
mapped read-only and never resized. Growing the database means extending the
file, initializing a new segment through the writer path, and adding a new
read-only mapping. Existing mapping addresses remain stable.

An address is a 64-bit file offset. Resolution derives the segment and local
offset, validates alignment and record bounds, and returns a slice tied to a
mapping guard owned by the read transaction.

The engine must not create Rust mutable references to bytes concurrently
visible through shared mappings. The safe initial strategy is:

- readers use read-only mappings;
- the writer uses positional file writes into unpublished extents;
- newly written bytes are flushed before publication; and
- mappings are created/refreshed only after the relevant file range exists.

Using a writable mmap can be evaluated later, but only behind a small audited
unsafe boundary with an aliasing argument. The crate currently forbids unsafe
code until that boundary exists and is reviewed. mmap libraries necessarily
encapsulate OS-level unsafe code, but the database API need not expose it.

### Allocation

The data arena begins after a reserved header and never uses offset zero.
Allocation has fixed-size classes for inner nodes and aligned variable-size
extents for leaves/keys/values. Initially, commits append to the tail. This
wastes space but makes crash atomicity easy to establish.

Reclamation is added after the commit protocol is proven:

1. mutation records which old extents cease to be reachable from the new root;
2. successful publication tags them with the replacing sequence;
3. the reader registry identifies the oldest active sequence;
4. safe extents move to persistent free lists; and
5. future transactions may allocate from those lists.

Before reuse, recovery must be able to distinguish allocated, retired, and
free extents. Free-list changes are themselves transactional. A reachability
scan/rebuild tool is required as a repair path even if normal open is O(1).

## Atomic commit and recovery

`META` contains two checksummed superblocks. Each contains at least:

```text
magic | format_version | feature_flags | generation | commit_sequence |
root_address | entry_count | durable_data_tail | allocator_root | checksum
```

The valid slot with the highest generation is authoritative. A commit follows
this ordering:

1. Finish the private overlay and enumerate new and retired extents.
2. Encode and validate all new records in transaction-owned memory.
3. Reserve DATA space and write records; none are reachable from current META.
4. Flush the written DATA ranges (and file length metadata when it grew).
5. Write the inactive META slot with generation + 1 and the new root.
6. Flush META durably.
7. Publish the same `(sequence, root)` to in-process readers with release
   ordering.
8. Retire replaced extents; do not reuse them until reader epochs permit it.

A crash before step 6 leaves the old superblock authoritative and new tail
bytes unreachable. A crash after step 6 exposes the complete new root. Open
validates both superblocks, selects the newest valid one, checks that its root
and tail are in bounds, and ignores/truncates uncommitted tail space where
safe.

Reusing old extents complicates this protocol because a crash must not expose
overwritten bytes through the old superblock. Therefore the first durable
engine is append-only. Reuse requires either a redo/undo log or an allocator
scheme that does not overwrite anything reachable from either superblock
until the publication point and reader grace period have both passed.

The durability contract must distinguish:

- `commit`: visible atomically in this process;
- `commit_durable`: visible and synced against process/OS crash assumptions;
- optional relaxed/group commit modes.

No durability claim is accepted without fault-injection tests terminating the
process after each write and flush boundary.

## Read path and zero-copy API

A point read performs:

1. acquire the current root snapshot and mapping guard;
2. traverse fixed-size inner nodes by checked in-place decoding;
3. compare compressed prefixes against mapped key bytes;
4. find a child using binary search over at most 32 sorted digits;
5. validate the leaf's complete key; and
6. return a value view borrowing from the read transaction.

The API should make lifetimes explicit:

```text
let read = db.read();
if let Some(value) = read.get(b"key")? {
    consume(value.as_bytes());
}
```

`value` cannot outlive `read`. A convenience `get_owned` may copy into a
`Vec<u8>`. Iterators similarly borrow the read transaction and load values
lazily so key-only scans do not touch value pages.

Zero-copy is impossible when a value crosses non-contiguous segments, is
compressed/encrypted, or the caller needs ownership beyond the transaction.
These cases must be explicit in the type/API or documented as copies.

## Validation and corruption handling

Mapped bytes are untrusted. Decoding validates, without allocation where
possible:

- record type, format version, size, alignment, and checksum;
- every referenced range against the durable tail and mapped segment;
- stride bounds and digit fit;
- child count at most 32 and strictly sorted digits;
- prefix bit bounds against the referenced key;
- terminal and leaf key relationships; and
- cycles or unreasonable depth during offline full validation.

Normal reads validate local records. An explicit full-check tool walks the
reachable graph, reports leaks/overlaps, and can rebuild allocator metadata.
Panics are programming bugs; malformed storage returns a corruption error.

## Decisions still requiring prototypes

1. **Node size and packing.** Benchmark 512-byte, 1-KiB, and page-packed nodes
   with real prefix-heavy and random workloads before freezing the format.
2. **Prefix ownership.** Compare representative-key references against inline
   prefix bytes plus overflow records; include deletion and reclamation cost.
3. **Mapped value shape.** Decide whether large cross-segment values are
   rejected, chunked, separately mapped, or copied by the convenience API.
4. **Reader registry.** Compare a mutex-protected registry at transaction open/
   close with per-thread epoch slots. Point reads themselves need no registry
   interaction after opening.
5. **Checksum granularity.** Per record catches localized corruption but costs
   CPU; per page/segment is cheaper but broadens recovery units.
6. **Reuse protocol.** Choose logging or delayed-generation free lists only
   after append-only crash recovery is exhaustively tested.
7. **Caching.** mmap delegates clean pages to the OS. Add a decoded-node cache
   only if profiling shows checked decoding is material; do not duplicate the
   page cache by default.
8. **Commit batching.** Establish a clear durability/latency contract before
   group commit or relaxed syncing.
9. **Optimistic writers.** Prototype root-descriptor CAS with epoch-based
   reclamation, then compare serialized commits, prepare-in-parallel commits,
   and whole-root retries under contention. Do not attempt per-node CAS unless
   that experiment demonstrates a clear need.

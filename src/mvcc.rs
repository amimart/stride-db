//! Root-snapshot MVCC metadata.

use crate::{Address, CommitSeq};

/// A consistent database state captured when a read transaction starts.
///
/// The future read transaction owns the mapping guard and epoch pin that make
/// all addresses reachable from this root safe to borrow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootSnapshot {
    sequence: CommitSeq,
    root: Address,
}

impl RootSnapshot {
    /// Creates snapshot metadata from an atomically published root pair.
    #[must_use]
    pub const fn new(sequence: CommitSeq, root: Address) -> Self {
        Self { sequence, root }
    }

    /// The internal sequence visible to the reader.
    #[must_use]
    pub const fn sequence(self) -> CommitSeq {
        self.sequence
    }

    /// The immutable SART root visible to the reader.
    #[must_use]
    pub const fn root(self) -> Address {
        self.root
    }
}

/// A retired extent that must remain mapped until older readers finish.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetiredExtent {
    /// First byte of the extent.
    pub address: Address,
    /// Length in bytes.
    pub length: u32,
    /// The extent is reusable once all readers at or below this sequence end.
    pub retire_after: CommitSeq,
}

impl RetiredExtent {
    /// Returns whether the extent can be recycled given the oldest reader.
    #[must_use]
    pub fn is_reclaimable(self, oldest_reader: Option<CommitSeq>) -> bool {
        oldest_reader.is_none_or(|oldest| oldest > self.retire_after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extent_waits_for_reader_of_retired_generation() {
        let extent = RetiredExtent {
            address: Address::from_u64(4096),
            length: 1024,
            retire_after: CommitSeq::from_u64(7),
        };

        assert!(!extent.is_reclaimable(Some(CommitSeq::from_u64(7))));
        assert!(extent.is_reclaimable(Some(CommitSeq::from_u64(8))));
        assert!(extent.is_reclaimable(None));
    }
}

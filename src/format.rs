//! Fundamental on-disk identifiers.

use core::fmt;

/// Byte offset of a record in the data arena.
///
/// Zero is reserved as a null address. Records are addressed by offset rather
/// than by process pointers so mappings may move between processes.
#[derive(Clone, Copy, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct Address(u64);

impl Address {
    /// The null address.
    pub const NULL: Self = Self(0);

    /// Creates an address from its persisted representation.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Returns the persisted representation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns whether this is the reserved null address.
    #[must_use]
    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({:#x})", self.0)
    }
}

/// Monotonic internal commit sequence used for snapshot visibility.
///
/// It is not a user-facing historical database version.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct CommitSeq(u64);

impl CommitSeq {
    /// The state of a newly created, empty database.
    pub const ZERO: Self = Self(0);

    /// Creates a sequence from its persisted representation.
    #[must_use]
    pub const fn from_u64(value: u64) -> Self {
        Self(value)
    }

    /// Returns the persisted representation.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next sequence, or `None` on exhaustion.
    #[must_use]
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_address_is_reserved() {
        assert!(Address::NULL.is_null());
        assert!(!Address::from_u64(8).is_null());
    }

    #[test]
    fn commit_sequence_detects_exhaustion() {
        assert_eq!(CommitSeq::ZERO.checked_next(), Some(CommitSeq::from_u64(1)));
        assert_eq!(CommitSeq::from_u64(u64::MAX).checked_next(), None);
    }
}

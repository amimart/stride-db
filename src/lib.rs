//! An experimental mmap-backed MVCC key-value store using SART.
//!
//! The crate is an architectural scaffold. It deliberately exposes no `Db`
//! implementation until crash recovery and mapped-slice lifetimes are proven.

#![forbid(unsafe_code)]

pub mod format;
pub mod mvcc;
pub mod sart;

pub use format::{Address, CommitSeq};

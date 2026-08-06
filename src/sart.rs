//! Logical SART constraints, independent of persistence encoding.

/// Maximum number of digit children in an inner node.
pub const NODE_CAPACITY: usize = 32;

/// Minimum number of key bits consumed by a routing digit.
pub const MIN_STRIDE_BITS: u8 = 1;

/// Maximum number of key bits consumed by a routing digit.
pub const MAX_STRIDE_BITS: u8 = 16;

/// Target occupancy after reducing an overflowing node's stride.
pub const RESHAPE_TARGET: usize = NODE_CAPACITY / 2;

/// Returns whether a stride is representable by the SART format.
#[must_use]
pub const fn valid_stride(stride: u8) -> bool {
    stride >= MIN_STRIDE_BITS && stride <= MAX_STRIDE_BITS
}

/// Returns whether `digit` fits in `stride` bits.
#[must_use]
pub const fn digit_fits(stride: u8, digit: u16) -> bool {
    if !valid_stride(stride) {
        return false;
    }
    if stride == 16 {
        return true;
    }
    digit < (1_u16 << stride)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_digit_width() {
        assert!(digit_fits(1, 1));
        assert!(!digit_fits(1, 2));
        assert!(digit_fits(16, u16::MAX));
        assert!(!digit_fits(0, 0));
    }
}

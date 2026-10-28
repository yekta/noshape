//! A fast hasher for small integer keys (Firefox's FxHash: one multiply per word); the standard SipHash costs
//! more than the work around it here.

use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default, Clone, Copy)]
pub(crate) struct FxHasher(u64);

impl Hasher for FxHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut w = [0u8; 8];
            w[..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(w));
        }
    }

    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }

    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
    }

    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }
}

pub(crate) type HashMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<FxHasher>>;
pub(crate) type HashSet<K> = std::collections::HashSet<K, BuildHasherDefault<FxHasher>>;

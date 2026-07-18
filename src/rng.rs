// rng.rs — deterministic PRNG core: xorshift64 (`Rng`) plus the named-channel
// hashing scheme (`h64`/`channel`) that keeps combat/AI/worldgen random
// streams from ever perturbing each other, and the FNV-1a byte hasher used
// by state/world hashing.

// ---------- RNG (xorshift64) ----------
pub(crate) struct Rng(pub(crate) u64);
impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    /// random in [lo, hi)
    pub(crate) fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + (self.next() % ((hi - lo) as u64)) as i32
    }
    pub(crate) fn chance(&mut self, num: u64, den: u64) -> bool {
        self.next() % den < num
    }
}

// ---------- Channel RNG ----------
// FNV-1a(64) over the seed bytes plus unit-separated tags, then a splitmix-style
// finalizer. Named channels isolate random streams: combat rolls and AI wander
// can never perturb worldgen, so a seed always generates the same world.
// This hash is a public API once golden fixtures exist: changing any constant
// or the tag scheme invalidates every seed in the wild (MAJOR version bump).
pub(crate) fn h64(seed: u64, tags: &[&str]) -> u64 {
    const PRIME: u64 = 0x100_0000_01b3;
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in seed.to_le_bytes() {
        h = (h ^ b as u64).wrapping_mul(PRIME);
    }
    for t in tags {
        h = (h ^ 0x1f).wrapping_mul(PRIME); // unit separator, golem-style
        for b in t.bytes() {
            h = (h ^ b as u64).wrapping_mul(PRIME);
        }
    }
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;
    h = h.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    h ^= h >> 33;
    h
}

pub(crate) fn channel(seed: u64, tags: &[&str]) -> Rng {
    Rng::new(h64(seed, tags))
}

pub(crate) fn fnv_bytes(h: u64, bytes: &[u8]) -> u64 {
    let mut h = h;
    for &b in bytes {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    h
}

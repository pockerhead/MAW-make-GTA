use rand_chacha::{
    ChaCha8Rng,
    rand_core::{Rng, SeedableRng},
};

pub(crate) const GRID_CORE_POS_X: u64 = 1;
pub(crate) const GRID_CORE_NEG_X: u64 = 2;
pub(crate) const GRID_OUTER_POS_X: u64 = 3;
pub(crate) const GRID_OUTER_NEG_X: u64 = 4;
pub(crate) const GRID_CORE_POS_Z: u64 = 5;
pub(crate) const GRID_CORE_NEG_Z: u64 = 6;
pub(crate) const GRID_OUTER_POS_Z: u64 = 7;
pub(crate) const GRID_OUTER_NEG_Z: u64 = 8;
pub(crate) const JITTER: u64 = 9;
pub(crate) const DISTRICTS: u64 = 10;
pub(crate) const EDGES: u64 = 11;
pub(crate) const PARKS: u64 = 12;
pub(crate) const LOTS: u64 = 13;
pub(crate) const BUILDINGS: u64 = 14;
pub(crate) const GANGS: u64 = 15;

// FNV-1a is fixed here so RNG substreams are independent of Rust's hasher.
pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        h = (h ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
    }
    h
}

pub(crate) fn stream(seed: u64, tag: u64, cell: u64) -> ChaCha8Rng {
    let mut bytes = [0_u8; 24];
    bytes[..8].copy_from_slice(&seed.to_le_bytes());
    bytes[8..16].copy_from_slice(&tag.to_le_bytes());
    bytes[16..].copy_from_slice(&cell.to_le_bytes());
    ChaCha8Rng::seed_from_u64(fnv1a64(&bytes))
}

pub(crate) fn unit_f32(rng: &mut impl Rng) -> f32 {
    (rng.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
}
pub(crate) fn range_f32(rng: &mut impl Rng, lo: f32, hi: f32) -> f32 {
    lo + (hi - lo) * unit_f32(rng)
}
pub(crate) fn range_u32(rng: &mut impl Rng, lo: u32, hi_incl: u32) -> u32 {
    lo + (rng.next_u64() % u64::from(hi_incl - lo + 1)) as u32
}
pub(crate) fn chance(rng: &mut impl Rng, p: f32) -> bool {
    unit_f32(rng) < p
}

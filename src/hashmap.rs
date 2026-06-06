const TABLE_SIZE: usize = 1 << 21;
const MASK: usize = TABLE_SIZE - 1;
const MAX_KEYS: usize = 1_000_000;
const EMPTY: u8 = 0x00;
const OCCUPIED: u8 = 0x01;

use rapidhash::v3::{rapidhash_v3_seeded, DEFAULT_RAPID_SECRETS};

#[inline(always)]
fn hash_key(key: &[u8; 16]) -> u64 {
    rapidhash_v3_seeded(key, &DEFAULT_RAPID_SECRETS)
}

#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct Slot {
    key:        [u8; 16],
    value:      [u8; 32],
    hash:       u64,
    value_len:  u8,
    status:     u8,
    probe_dist: u8,
    _pad:       [u8; 5],
}

const _: () = assert!(std::mem::size_of::<Slot>() == 64, "Slot must be exactly 64 bytes");

impl Slot {
    const fn empty() -> Self {
        Self {
            key: [0u8; 16], value: [0u8; 32], hash: 0,
            value_len: 0, status: EMPTY, probe_dist: 0, _pad: [0u8; 5],
        }
    }
}

static mut TABLE: [Slot; TABLE_SIZE] = [Slot::empty(); TABLE_SIZE];
static mut COUNT: usize = 0;

pub fn init() {
    unsafe {
        COUNT = 0;
        let ptr = std::ptr::addr_of_mut!(TABLE) as *mut u8;
        std::ptr::write_bytes(ptr, 0, TABLE_SIZE * std::mem::size_of::<Slot>());
    }
}
pub fn insert(key: &[u8; 16], value: &[u8], value_len: u8) -> i32 {
    if unsafe { COUNT } >= MAX_KEYS { return -1; }
    let hash = hash_key(key);
    let mut index = (hash as usize) & MASK;
    let mut incoming = Slot {
        key: *key,
        value: { let mut v = [0u8; 32]; let n = (value_len as usize).min(32); v[..n].copy_from_slice(&value[..n]); v },
        hash, value_len, status: OCCUPIED, probe_dist: 0, _pad: [0u8; 5],
    };
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let next_ptr = core::ptr::addr_of!(TABLE)
                .cast::<Slot>()
                .add((index + 1) & MASK)
                .cast::<i8>();
            core::arch::x86_64::_mm_prefetch(next_ptr, core::arch::x86_64::_MM_HINT_T0);
        }
        let slot = unsafe { &mut TABLE[index] };
        if slot.status == EMPTY { *slot = incoming; unsafe { COUNT += 1; } return 0; }
        if slot.hash == hash && slot.key == *key {
            let n = (value_len as usize).min(32);
            slot.value[..n].copy_from_slice(&value[..n]);
            slot.value_len = value_len;
            return 1;
        }
        if slot.probe_dist < incoming.probe_dist { std::mem::swap(slot, &mut incoming); }
        index = (index + 1) & MASK;
        incoming.probe_dist = incoming.probe_dist.saturating_add(1);
    }
}

pub fn get(key: &[u8; 16], out: &mut [u8; 32], value_len: &mut u8) -> i32 {
    let hash = hash_key(key);
    let mut index = (hash as usize) & MASK;
    let mut dist: u8 = 0;
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            let next_ptr = core::ptr::addr_of!(TABLE)
                .cast::<Slot>()
                .add((index + 1) & MASK)
                .cast::<i8>();
            core::arch::x86_64::_mm_prefetch(next_ptr, core::arch::x86_64::_MM_HINT_T0);
        }
        let slot = unsafe { &TABLE[index] };
        if slot.status == EMPTY { return -1; }
        if slot.hash == hash && slot.key == *key { *out = slot.value; *value_len = slot.value_len; return 0; }
        if slot.probe_dist < dist { return -1; }
        index = (index + 1) & MASK;
        dist = dist.saturating_add(1);
    }
}

pub fn delete(key: &[u8; 16]) -> i32 {
    let hash = hash_key(key);
    let mut index = (hash as usize) & MASK;
    let mut dist: u8 = 0;
    loop {
        let slot = unsafe { &TABLE[index] };
        if slot.status == EMPTY { return -1; }
        if slot.hash == hash && slot.key == *key { break; }
        if slot.probe_dist < dist { return -1; }
        index = (index + 1) & MASK;
        dist = dist.saturating_add(1);
    }
    loop {
        let next = (index + 1) & MASK;
        let next_slot = unsafe { &TABLE[next] };
        if next_slot.status == EMPTY || next_slot.probe_dist == 0 {
            unsafe { TABLE[index] = Slot::empty(); }
            break;
        }
        unsafe { TABLE[index] = TABLE[next]; TABLE[index].probe_dist -= 1; }
        index = next;
    }
    unsafe { COUNT -= 1; }
    0
}

#[inline]
pub fn size() -> usize { unsafe { COUNT } }

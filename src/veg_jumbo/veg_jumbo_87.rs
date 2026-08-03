use rapidhash::v3::{DEFAULT_RAPID_SECRETS, rapidhash_v3_seeded};

pub const TABLE_SIZE: usize = 1 << 21;
pub const MASK: u32 = (TABLE_SIZE as u32) - 1;
pub const MAX_KEYS: usize = 1835008;

pub const EMPTY: u8 = 0x00;
pub const OCCUPIED: u8 = 0x01;

pub const KEY_SIZE: usize = 16;
pub const VALUE_SIZE: usize = 100;

#[repr(C, align(64))]
#[derive(Clone, Copy)]
struct Slot {
    key: [u8; KEY_SIZE],
    value: [u8; VALUE_SIZE],
    hash: u64,
    value_len: u8,
    status: u8,
    probe_dist: u16,
}

impl Slot {
    pub const EMPTY: Self = Self {
        key: [0; KEY_SIZE],
        value: [0; VALUE_SIZE],
        hash: 0,
        value_len: 0,
        status: EMPTY,
        probe_dist: 0,
    };
}

pub struct Vegosh {
    slots: [Slot; TABLE_SIZE],
    count: usize,
}

impl Vegosh {
    pub const fn new() -> Self {
        Self {
            slots: [Slot::EMPTY; TABLE_SIZE],
            count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    Updated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableFull;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotFound;

#[inline(always)]
fn hash_key(key: &[u8; 16]) -> u64 {
    rapidhash_v3_seeded(key, &DEFAULT_RAPID_SECRETS)
}
// Resets the global table to empty and returns a pointer to it.
// Call once at startup before using insert/get/delete_key.

#[inline(always)]
pub fn init(table: &mut Vegosh) {
    table.count = 0;
    table.slots.fill(Slot::EMPTY);
}
// Inserts a key/value pair, or updates the value if the key already exists.
// Uses Robin Hood hashing with linear probing: when probing finds a slot
// that's "richer" (closer to its ideal index) than the incoming entry,
// the two swap places, so no single key ever ends up displaced too far.
//
#[inline(always)]
pub fn insert(
    table: &mut Vegosh,
    key: &[u8; 16],
    value: &[u8; 100],
    value_len: u8,
) -> Result<InsertOutcome, TableFull> {
    assert!((value_len as usize) <= VALUE_SIZE);

    if table.count >= MAX_KEYS {
        return Err(TableFull);
    }

    let hash = hash_key(key);
    let mut index: u32 = (hash as u32) & MASK; // ideal slot for this key, before any probing

    // Build the entry we're trying to place. This may get swapped out for
    // a "poorer" entry mid-probe (see the Robin Hood swap below), so it's
    // a mutable local, not written directly into the table yet.
    let mut incoming = Slot {
        key: *key,
        value: *value,
        hash,
        value_len,
        status: OCCUPIED,
        probe_dist: 0,
    };

    loop {
        //Prefetch two slots ahead so the next probe iteration's cache line
        // is already in flight by the time we need it.
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use core::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};

            let next_ptr = table
                .slots
                .as_ptr()
                .add(((index + 4) & MASK) as usize)
                .cast::<i8>();

            _mm_prefetch(next_ptr, _MM_HINT_T0);
        }

        let slot = &mut table.slots[index as usize];
        // Found an empty slot: place the entry here and we're done.
        if slot.status == EMPTY {
            *slot = incoming;
            table.count += 1;
            return Ok(InsertOutcome::Inserted);
        }
        // Check hash first (cheap), only fall through to the full key
        // comparison (memcmp) if the hash actually matches.
        if slot.hash == incoming.hash && slot.key == incoming.key {
            // Same key already exists: update its value in place.
            slot.value = incoming.value;
            slot.value_len = incoming.value_len;
            return Ok(InsertOutcome::Updated);
        }
        // Robin Hood swap: if the entry we're carrying has traveled further
        // from its ideal slot than the one currently occupying this slot,
        // steal this slot and carry the displaced entry onward instead.
        // This keeps probe distances balanced across the whole table.
        if incoming.probe_dist > slot.probe_dist {
            std::mem::swap(slot, &mut incoming);
        }

        index = (index + 1) & MASK;
        incoming.probe_dist += 1;
    }
}
// Looks up a key and copies its value into out_value / out_value_len.
//
// The probe_distance early-exit below is what makes Robin Hood lookups
// fast: if we've probed further than the slot in front of us has ever
// been displaced, the key we're looking for cannot exist anywhere later
// in the probe sequence, so we can stop instead of scanning the whole table.
#[inline(never)]
pub fn get(table: &Vegosh, key: &[u8; 16]) -> Option<([u8; 100], u8)> {
    let hash = hash_key(key);
    let mut index: u32 = (hash as u32) & MASK;
    let mut probe_dist: u16 = 0;

    loop {
        let slot = &table.slots[index as usize];
        // Hit an empty slot before finding the key: it's not in the table.
        if slot.status == EMPTY {
            return None;
        }
        // Found it — copy out the value and how much of it is valid.
        if slot.hash == hash && slot.key == *key {
            return Some((slot.value, slot.value_len));
        }
        // Robin Hood early-exit: this slot is "richer" than we've traveled,
        // so our key can't be further down the probe sequence.
        if probe_dist > slot.probe_dist {
            return None;
        }

        index = (index + 1) & MASK;
        probe_dist += 1;
    }
}
// Removes a key from the table using backward-shift deletion, which keeps
// the Robin Hood probe-distance invariant intact without needing tombstones.
//
// Phase 1: probe forward to find the slot holding this key (same logic as get()).
// Phase 2: shift each subsequent slot backward by one, decrementing its
// probe_distance, until we hit an empty slot or a slot that's already at
// its ideal position (probe_distance == 0) — that's where the "hole" closes.
//

#[inline(always)]
pub fn delete(table: &mut Vegosh, key: &[u8; 16]) -> Result<(), NotFound> {
    let hash = hash_key(key);
    let mut index: u32 = (hash as u32) & MASK;
    let mut probe_dist: u16 = 0;
    // Phase 1: locate the slot holding this key.
    loop {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use core::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};

            let next_ptr = table
                .slots
                .as_ptr()
                .add(((index + 4) & MASK) as usize)
                .cast::<i8>();

            _mm_prefetch(next_ptr, _MM_HINT_T0);
        }

        let slot = &table.slots[index as usize];

        if slot.status == EMPTY {
            return Err(NotFound);
        }

        if slot.hash == hash && slot.key == *key {
            break;
        }

        if slot.probe_dist < probe_dist {
            return Err(NotFound);
        }

        index = (index + 1) & MASK;
        probe_dist += 1;
    }
    // Phase 2: backward-shift every following displaced entry into the gap
    // we just opened, one slot at a time, until the chain naturally ends.
    loop {
        let next = (index + 1) & MASK;

        if table.slots[next as usize].status == EMPTY || table.slots[next as usize].probe_dist == 0
        {
            table.slots[index as usize] = Slot::EMPTY;
            break;
        }

        table.slots[index as usize] = table.slots[next as usize];
        table.slots[index as usize].probe_dist -= 1;

        index = next;
    }

    table.count -= 1;
    Ok(())
}
// Returns the current number of occupied entries in the table.
#[inline(always)]
pub fn size(table: &Vegosh) -> usize {
    table.count
}
// Wipes every slot back to EMPTY and resets the entry count to zero.
// Table remains usable afterward — this doesn't free or reallocate anything.
#[inline(always)]
pub fn clear(table: &mut Vegosh) {
    table.slots.fill(Slot::EMPTY);
    table.count = 0;
}

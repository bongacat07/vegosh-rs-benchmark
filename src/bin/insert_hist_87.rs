use rand::seq::SliceRandom;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::u64;
use veg_hashmap::vegosh::vegosh_87::*;

const A: u64 = 0x9e3779b97f4a7c15;
const B: u64 = 0xd1b54a32d192ed03;
const VALUE: [u8; 32] = [
    0x9f, 0x4a, 0x7c, 0x2e, 0xd1, 0x83, 0x56, 0xb9, 0x14, 0xea, 0x67, 0x3d, 0x80, 0xc5, 0x29, 0xf2,
    0x71, 0x0b, 0xa8, 0x4f, 0xde, 0x95, 0x32, 0x6c, 0x58, 0xe1, 0x13, 0xaf, 0x7b, 0xc4, 0x90, 0x2d,
];

fn create(x: u64) -> u64 {
    A * x + B
}

fn generate_key(i: u64) -> u128 {
    ((create(i) as u128) << 64) | (create(i ^ 0x9e3779b97f4a7c15) as u128)
}

fn generate_keys_table() -> Vec<u128> {
    let mut keys = Vec::with_capacity(MAX_KEYS as usize);

    for i in 0..MAX_KEYS {
        keys.push(generate_key(i as u64));
    }

    let mut rng = rand::rng();
    keys.shuffle(&mut rng);

    keys
}

fn insert_hist(table: &mut Vegosh, keys: &[u128], hist: &mut [u32; 4096]) {
    clear(table);

    for key in keys.iter().take(MAX_KEYS as usize) {
        let key_bytes = key.to_le_bytes();
        let _ = insert_modified(table, &key_bytes, &VALUE, VALUE.len() as u8, hist);
    }
}

static mut TABLE: Vegosh = Vegosh::new();

fn main() -> std::io::Result<()> {
    let keys = generate_keys_table();
    let table: &mut Vegosh = unsafe { &mut *core::ptr::addr_of_mut!(TABLE) };

    init(table);

    let file = File::create("hist_87.csv")?;
    let mut writer = BufWriter::new(file);

    let mut landing_hist = [0u32; 4096];
    insert_hist(table, &keys, &mut landing_hist);
    let mut snapshot_hist = [0u32; 4096];
    probe_dist_snapshot(table, &mut snapshot_hist);
    writeln!(writer, "landings")?;
    for (dist, count) in landing_hist.iter().enumerate() {
        if *count != 0 {
            writeln!(writer, "{},{}", dist, count)?;
        }
    }
    writeln!(writer, "snapshot")?;
    for (dist, count) in snapshot_hist.iter().enumerate() {
        if *count != 0 {
            writeln!(writer, "{},{}", dist, count)?;
        }
    }

    Ok(())
}

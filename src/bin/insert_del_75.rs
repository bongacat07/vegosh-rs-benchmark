use std::collections::HashSet;
use std::fs::File;
use std::io::{BufWriter, Write};
use veg_hashmap::vegosh::vegosh_75::*;

const BATCH_SIZE: usize = 10_000;
const PROBE_THRESHOLD: u16 = 8;
const MAX_BATCHES: usize = 1_000_000;
const A: u64 = 0x9e3779b97f4a7c15;
const B: u64 = 0xd1b54a32d192ed03;
const VALUE: [u8; 32] = [
    0x9f, 0x4a, 0x7c, 0x2e, 0xd1, 0x83, 0x56, 0xb9, 0x14, 0xea, 0x67, 0x3d, 0x80, 0xc5, 0x29, 0xf2,
    0x71, 0x0b, 0xa8, 0x4f, 0xde, 0x95, 0x32, 0x6c, 0x58, 0xe1, 0x13, 0xaf, 0x7b, 0xc4, 0x90, 0x2d,
];

struct LiveKeys {
    key_num: u32,
    keys: HashSet<u128>,
}

impl LiveKeys {
    fn new() -> Self {
        Self {
            key_num: 0,
            keys: HashSet::with_capacity(MAX_KEYS),
        }
    }

    fn add_key(&mut self, key: u128) {
        self.keys.insert(key);
    }

    fn remove_key(&mut self, key: u128) {
        assert!(self.keys.remove(&key), "evicted key not found in live pool");
    }
}

fn create(x: u64) -> u64 {
    A * x + B
}

fn generate_key(i: u64) -> u128 {
    ((create(i) as u128) << 64) | (create(i ^ 0x9e3779b97f4a7c15) as u128)
}

fn snapshot(
    table: &Vegosh,
    hist: &mut [u32; 4096],
    writer: &mut BufWriter<File>,
    label: &str,
) -> std::io::Result<()> {
    hist.fill(0);
    probe_dist_snapshot(table, hist);
    writeln!(writer, "snapshot {}", label)?;
    for (dist, count) in hist.iter().enumerate() {
        if *count != 0 {
            writeln!(writer, "{},{}", dist, count)?;
        }
    }
    Ok(())
}

fn run_batch_evict(table: &mut Vegosh, threshold: u16, mut writer: BufWriter<File>) {
    let mut pool = LiveKeys::new();
    let mut hist = [0u32; 4096];
    let mut batch_num: usize = 0;

    loop {
        if size(table) >= MAX_KEYS {
            println!(
                "reached MAX_KEYS ({}) after {} batches",
                MAX_KEYS, batch_num
            );
            break;
        }
        if batch_num >= MAX_BATCHES {
            eprintln!(
                "WARNING: hit MAX_BATCHES ({}) without reaching MAX_KEYS; table size = {}.",
                MAX_BATCHES,
                size(table)
            );
            break;
        }

        let mut inserted_this_batch = 0usize;
        while inserted_this_batch < BATCH_SIZE && size(table) < MAX_KEYS {
            let key = generate_key(pool.key_num as u64);
            insert(table, &key.to_be_bytes(), &VALUE, VALUE_SIZE as u8)
                .expect("table full despite size() check");
            pool.add_key(key);
            pool.key_num += 1;
            inserted_this_batch += 1;
        }

        batch_num += 1;

        let offenders = keys_over_probe_dist(table, threshold);
        for key_bytes in &offenders {
            delete(table, key_bytes).expect("offender key vanished between scan and delete");
            pool.remove_key(u128::from_be_bytes(*key_bytes));
        }

        println!(
            "batch {}: inserted {}, evicted {}, table size {}",
            batch_num,
            inserted_this_batch,
            offenders.len(),
            size(table)
        );

        snapshot(table, &mut hist, &mut writer, &batch_num.to_string())
            .expect("snapshot write failed");
    }
}

static mut TABLE: Vegosh = Vegosh::new();

fn main() -> std::io::Result<()> {
    let table: &mut Vegosh = unsafe { &mut *core::ptr::addr_of_mut!(TABLE) };
    init(table);

    let file = File::create("fuzz_75_evict.csv")?;
    let writer = BufWriter::new(file);

    run_batch_evict(table, PROBE_THRESHOLD, writer);

    Ok(())
}

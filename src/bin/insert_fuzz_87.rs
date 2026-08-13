use rand::rngs::SmallRng;
use rand::{RngExt, SeedableRng};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::u64;
use veg_hashmap::vegosh::vegosh_87::*;

const OPS: usize = 500_000_000;
const INSERT_NUMERATOR: u32 = 67;
const INSERT_DENOMINATOR: u32 = 100;
const A: u64 = 0x9e3779b97f4a7c15;
const B: u64 = 0xd1b54a32d192ed03;
const VALUE: [u8; 32] = [
    0x9f, 0x4a, 0x7c, 0x2e, 0xd1, 0x83, 0x56, 0xb9, 0x14, 0xea, 0x67, 0x3d, 0x80, 0xc5, 0x29, 0xf2,
    0x71, 0x0b, 0xa8, 0x4f, 0xde, 0x95, 0x32, 0x6c, 0x58, 0xe1, 0x13, 0xaf, 0x7b, 0xc4, 0x90, 0x2d,
];

struct LiveKeys {
    key_num: u32,
    keys: Vec<u128>,
}

impl LiveKeys {
    fn new() -> Self {
        Self {
            key_num: 0,
            keys: Vec::with_capacity(MAX_KEYS),
        }
    }

    fn add_key(&mut self, key: u128) {
        self.keys.push(key);
    }

    fn delete_random_key<R: rand::RngExt + ?Sized>(&mut self, rng: &mut R) -> u128 {
        let idx = rng.random_range(0..self.keys.len());
        self.keys.swap_remove(idx)
    }
}
fn create(x: u64) -> u64 {
    A * x + B
}

fn generate_key(i: u64) -> u128 {
    ((create(i) as u128) << 64) | (create(i ^ 0x9e3779b97f4a7c15) as u128)
}

fn generate_workload(seed: u64) -> Vec<u8> {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut workload = Vec::with_capacity(OPS);
    for _ in 0..OPS {
        if rng.random_ratio(INSERT_NUMERATOR, INSERT_DENOMINATOR) {
            workload.push(b'I');
        } else {
            workload.push(b'D');
        }
    }
    workload
}

const CHECKPOINTS: [(usize, u8); 7] = [
    (OPS * 25 / 100, 25),
    (OPS * 50 / 100, 50),
    (OPS * 75 / 100, 75),
    (OPS * 90 / 100, 90),
    (OPS * 95 / 100, 95),
    (OPS * 99 / 100, 99),
    (OPS, 100),
];

fn snapshot(
    table: &Vegosh,
    hist: &mut [u32; 4096],
    writer: &mut BufWriter<File>,
    period: u8,
) -> std::io::Result<()> {
    hist.fill(0);
    probe_dist_snapshot(table, hist);
    writeln!(writer, "snapshot {}", period)?;
    for (dist, count) in hist.iter().enumerate() {
        if *count != 0 {
            writeln!(writer, "{},{}", dist, count)?;
        }
    }
    Ok(())
}

fn run_insert_hist(seed: u64, workload: Vec<u8>, table: &mut Vegosh, mut writer: BufWriter<File>) {
    let mut pool: LiveKeys = LiveKeys::new();
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut hist = [0u32; 4096];
    let mut next_checkpoint = 0usize;

    for (i, work) in workload.iter().enumerate() {
        if work == &b'I' {
            if pool.keys.len() >= MAX_KEYS {
                let key = pool.delete_random_key(&mut rng);
                delete(table, &key.to_be_bytes())
                    .expect("key was tracked in pool but missing from table");
            } else {
                let key = generate_key(pool.key_num as u64);
                insert(table, &key.to_be_bytes(), &VALUE, VALUE_SIZE as u8)
                    .expect("table full despite pool cap enforcement");
                pool.add_key(key);
                pool.key_num += 1;
            }
        } else {
            if pool.keys.len() == 0 {
                let key = generate_key(pool.key_num as u64);
                insert(table, &key.to_be_bytes(), &VALUE, VALUE_SIZE as u8)
                    .expect("table full despite pool cap enforcement");
                pool.add_key(key);
                pool.key_num += 1;
            } else {
                let key = pool.delete_random_key(&mut rng);
                delete(table, &key.to_be_bytes())
                    .expect("key was tracked in pool but missing from table");
            }
        }

        let ops_done = i + 1;
        if next_checkpoint < CHECKPOINTS.len() && ops_done == CHECKPOINTS[next_checkpoint].0 {
            let period = CHECKPOINTS[next_checkpoint].1;
            snapshot(table, &mut hist, &mut writer, period).expect("snapshot write failed");
            next_checkpoint += 1;
        }
    }
}

static mut TABLE: Vegosh = Vegosh::new();

fn main() -> std::io::Result<()> {
    let seed: u64 = std::env::args()
        .nth(1)
        .expect("usage: <binary> <seed>")
        .parse()
        .expect("seed must be a valid u64");

    let table: &mut Vegosh = unsafe { &mut *core::ptr::addr_of_mut!(TABLE) };
    init(table);

    let workload = generate_workload(seed);

    let file = File::create(format!("fuzz_87_{:03}.csv", seed))?;
    let writer = BufWriter::new(file);

    run_insert_hist(seed, workload, table, writer);

    Ok(())
}

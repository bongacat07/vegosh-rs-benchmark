use rand::rng;
use rand::seq::{IndexedRandom, SliceRandom};
use std::env;
use std::u64;

use hashbrown::HashMap;

const A: u64 = 0x9e3779b97f4a7c15;
const B: u64 = 0xd1b54a32d192ed03;
const VALUE: [u8; 100] = [
    0x9f, 0x4a, 0x7c, 0x2e, 0xd1, 0x83, 0x56, 0xb9, 0x14, 0xea, 0x67, 0x3d, 0x80, 0xc5, 0x29, 0xf2,
    0x71, 0x0b, 0xa8, 0x4f, 0xde, 0x95, 0x32, 0x6c, 0x58, 0xe1, 0x13, 0xaf, 0x7b, 0xc4, 0x90, 0x2d,
    0x6e, 0x1a, 0xf5, 0x8c, 0x3b, 0xd2, 0x07, 0x49, 0xe3, 0x78, 0x9a, 0x51, 0xcb, 0x60, 0x24, 0xbd,
    0x8f, 0x15, 0xaa, 0x73, 0x4c, 0xe2, 0x9b, 0x3f, 0x06, 0x5c, 0xd8, 0x61, 0xb4, 0x7e, 0x23, 0x92,
    0x57, 0x0d, 0x81, 0xc9, 0xbe, 0x43, 0xa6, 0x1f, 0x72, 0xdb, 0x54, 0x8e, 0x3c, 0x97, 0x6b, 0x02,
    0xf0, 0x48, 0x1d, 0xa5, 0x79, 0xce, 0x33, 0x8b, 0x52, 0x96, 0x41, 0xb7, 0x0e, 0xd4, 0x68, 0xfa,
    0x35, 0x91, 0xbc, 0x27,
];

const MAX_KEYS: u64 = 1_000_000;

fn create(x: u64) -> u64 {
    A * x + B
}

fn generate_key(i: u64) -> u128 {
    ((create(i) as u128) << 64) | (create(i ^ 0x9e3779b97f4a7c15) as u128)
}

fn key_bytes(k: u128) -> [u8; 16] {
    k.to_le_bytes()
}

fn generate_table(ratio: f64, table: &mut HashMap<[u8; 16], [u8; 100]>) -> Vec<u128> {
    let mut real_keys: Vec<u128> = Vec::with_capacity(MAX_KEYS as usize);
    for i in 0..MAX_KEYS {
        let key = generate_key(i);
        let key_b = key_bytes(key);
        table.insert(key_b, VALUE);
        real_keys.push(key);
    }

    let mut rng = rng();

    if ratio >= 1.0 {
        real_keys.shuffle(&mut rng);
        return real_keys;
    }

    let real_count = (ratio * MAX_KEYS as f64).round() as usize;
    let fake_count = MAX_KEYS as usize - real_count;

    let real_key_array: Vec<u128> = real_keys.sample(&mut rng, real_count).cloned().collect();

    let fake_key_array: Vec<u128> = (MAX_KEYS..MAX_KEYS + fake_count as u64)
        .map(generate_key)
        .collect();

    let mut merged = Vec::with_capacity(MAX_KEYS as usize);
    merged.extend(real_key_array);
    merged.extend(fake_key_array);
    merged.shuffle(&mut rng);

    merged
}

struct Results {
    min: u64,
    max: u64,
    p25: u64,
    median: u64,
    p75: u64,
    p90: u64,
    p95: u64,
    p99: u64,
    mean: f64,
}

#[inline(always)]
fn rdtsc_begin() -> u64 {
    unsafe {
        core::arch::x86_64::_mm_lfence();
        core::arch::x86_64::_rdtsc()
    }
}

#[inline(always)]
fn rdtsc_end() -> u64 {
    unsafe {
        let mut _aux = 0u32;
        let t = core::arch::x86_64::__rdtscp(&mut _aux);
        core::arch::x86_64::_mm_lfence();
        t
    }
}

fn measure_overhead() -> u64 {
    let mut best = u64::MAX;

    for _ in 0..10_000 {
        let start = rdtsc_begin();
        let end = rdtsc_end();

        let cycles = end - start;
        if cycles < best {
            best = cycles;
        }
    }

    best
}

fn get_benchmark(table: &HashMap<[u8; 16], [u8; 100]>, overhead: u64, keys: &[u128]) -> Results {
    let mut samples = Vec::with_capacity(MAX_KEYS as usize);
    let mut total: u64 = 0;

    for key in keys.iter().take(MAX_KEYS as usize) {
        let key_bytes = key.to_le_bytes();

        let start = rdtsc_begin();
        let _rc = std::hint::black_box(table.get(&key_bytes));
        let end = rdtsc_end();

        let mut cycles = end - start;
        if cycles > overhead {
            cycles -= overhead;
        } else {
            cycles = 0;
        }

        samples.push(cycles);
        total += cycles;
    }

    if samples.is_empty() {
        panic!("No lookups performed.");
    }

    samples.sort_unstable();
    let n = samples.len();
    Results {
        min: samples[0],
        max: samples[n - 1],
        p25: samples[n * 25 / 100],
        median: samples[n / 2],
        p75: samples[n * 75 / 100],
        p90: samples[n * 90 / 100],
        p95: samples[n * 95 / 100],
        p99: samples[n * 99 / 100],
        mean: total as f64 / n as f64,
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let argc = args.len();
    let ratio: f64 = if argc < 2 {
        eprintln!("Usage: {} <ratio>", args[0]);
        std::process::exit(1);
    } else {
        match args[1].parse::<f64>() {
            Ok(num) => num,
            Err(_) => {
                eprintln!("Error: '{}' is not a valid float.", args[1]);
                std::process::exit(1);
            }
        }
    };

    let mut table: HashMap<[u8; 16], [u8; 100]> = HashMap::with_capacity(MAX_KEYS as usize);
    let keys = generate_table(ratio, &mut table);

    let overhead = measure_overhead();
    println!("Overhead: {}", overhead);
    println!("Max Keys: {}", MAX_KEYS);

    for run in 0..20 {
        let results = get_benchmark(&table, overhead, &keys);

        println!(
            "Run {:2}: min={} p25={} median={} p75={} p90={} p95={} p99={} max={} mean={:.2}",
            run,
            results.min,
            results.p25,
            results.median,
            results.p75,
            results.p90,
            results.p95,
            results.p99,
            results.max,
            results.mean,
        );
    }
}

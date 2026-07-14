use rand::rng;
use rand::seq::{IndexedRandom, SliceRandom};
use std::env;
use std::u64;

use vegosh::{Vegosh, clear, get, init, insert, size, vegosh::MAX_KEYS};

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

fn key_bytes(k: u128) -> [u8; 16] {
    k.to_le_bytes()
}

fn generate_table(ratio: f64, table: &mut Vegosh) -> Vec<u128> {
    init(table);

    let mut real_keys: Vec<u128> = Vec::with_capacity(MAX_KEYS as usize);
    for i in 0..MAX_KEYS as u64 {
        let key = generate_key(i);
        let key_b = key_bytes(key);
        insert(table, &key_b, &VALUE, VALUE.len() as u8);
        real_keys.push(key);
    }

    let mut rng = rng();

    if ratio >= 1.0 {
        real_keys.shuffle(&mut rng);
        return real_keys;
    }

    let real_count = (ratio * MAX_KEYS as f64).round() as usize;
    let fake_count = MAX_KEYS as usize - real_count;

    let real_key_array: Vec<u128> = real_keys
        .choose_multiple(&mut rng, real_count)
        .cloned()
        .collect();

    let fake_key_array: Vec<u128> = (MAX_KEYS as u64..MAX_KEYS as u64 + fake_count as u64)
        .map(generate_key)
        .collect();

    let mut merged = Vec::with_capacity(MAX_KEYS as usize);
    merged.extend(real_key_array);
    merged.extend(fake_key_array);
    merged.shuffle(&mut rng);

    merged
}

static mut TABLE: Vegosh = Vegosh::new();

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

fn get_benchmark(table: &Vegosh, overhead: u64, keys: &[u128]) -> Results {
    let mut samples = Vec::with_capacity(MAX_KEYS as usize);
    let mut total: u64 = 0;
    let mut out_value = [0u8; 32];
    let mut out_value_len: u8 = 0;

    for key in keys.iter().take(MAX_KEYS as usize) {
        let key_bytes = key.to_le_bytes();

        let start = rdtsc_begin();
        let _rc = get(table, &key_bytes, &mut out_value, &mut out_value_len);
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
    let table: &mut Vegosh = unsafe { &mut *core::ptr::addr_of_mut!(TABLE) };
    let keys = generate_table(ratio, table);

    let overhead = measure_overhead();
    println!("Overhead: {}", overhead);

    for run in 0..20 {
        let results = get_benchmark(table, overhead, &keys);

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

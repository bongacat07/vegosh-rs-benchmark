use std::u64;

use veg_hashmap::veg_jumbo::veg_jumbo_48::*;

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

    keys
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

fn insert_benchmark(table: &mut Vegosh, overhead: u64, keys: &[u128]) -> Results {
    clear(table);

    let mut samples = Vec::with_capacity(MAX_KEYS as usize);
    let mut total: u64 = 0;

    for (i, key) in keys.iter().take(MAX_KEYS as usize).enumerate() {
        let key_bytes = key.to_le_bytes(); // u128 -> [u8; 16], matches KEY_SIZE

        let start = rdtsc_begin();
        let rc = insert(table, &key_bytes, &VALUE, VALUE.len() as u8);
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
        panic!("No successful insertions.");
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
    let keys = generate_keys_table();
    let overhead = measure_overhead();
    println!("Overhead: {}", overhead);
    let table: &mut Vegosh = unsafe { &mut *core::ptr::addr_of_mut!(TABLE) };
    init(table);
    for run in 0..20 {
        let results = insert_benchmark(table, overhead, &keys);

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

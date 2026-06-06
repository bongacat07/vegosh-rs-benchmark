use hashbrown::HashMap;

const N: usize = 1_000_000;
const RUNS: usize = 5;

#[inline(always)]
fn rdtsc_start() -> u64 {
    unsafe {
        core::arch::x86_64::_mm_lfence();
        core::arch::x86_64::_rdtsc()
    }
}

#[inline(always)]
fn rdtsc_stop() -> u64 {
    unsafe {
        let mut _aux = 0u32;
        let t = core::arch::x86_64::__rdtscp(&mut _aux);
        core::arch::x86_64::_mm_lfence();
        t
    }
}

struct Xorshift128 {
    s: [u64; 2],
}

impl Xorshift128 {
    fn new(seed: u64) -> Self {
        Self { s: [seed ^ 0xdeadbeefcafe1234, seed.wrapping_add(1) ^ 0xabcdef0123456789] }
    }
    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        let mut s1 = self.s[0];
        let s0 = self.s[1];
        self.s[0] = s0;
        s1 ^= s1 << 23;
        self.s[1] = s1 ^ s0 ^ (s1 >> 18) ^ (s0 >> 5);
        self.s[1].wrapping_add(s0)
    }
    fn next_key(&mut self) -> [u8; 16] {
        let a = self.next_u64();
        let b = self.next_u64();
        let mut k = [0u8; 16];
        k[..8].copy_from_slice(&a.to_le_bytes());
        k[8..].copy_from_slice(&b.to_le_bytes());
        k
    }
}

struct Stats {
    min:    u64,
    max:    u64,
    median: u64,
    p99:    u64,
    mean:   f64,
}

fn compute_stats(samples: &mut Vec<u64>) -> Stats {
    samples.sort_unstable();
    let n = samples.len();
    Stats {
        min:    samples[0],
        max:    samples[n - 1],
        median: samples[n / 2],
        p99:    samples[(n as f64 * 0.99) as usize],
        mean:   samples.iter().sum::<u64>() as f64 / n as f64,
    }
}

fn run_once(keys: &[[u8; 16]; N], value: [u8; 32]) -> (Vec<u64>, u64) {
    // Pre-allocate to N to match our hashmap's pre-allocated static table
    let mut map: HashMap<[u8; 16], [u8; 32]> = HashMap::with_capacity(N);

    let mut per_insert = Vec::with_capacity(N);

    let wall_start = rdtsc_start();

    for key in keys.iter() {
        let t0 = rdtsc_start();
        map.insert(*key, value);
        let t1 = rdtsc_stop();
        per_insert.push(t1 - t0);
    }

    let wall_end = rdtsc_stop();

    (per_insert, wall_end - wall_start)
}

fn main() {
    let mut rng = Xorshift128::new(0x1234567890abcdef); // same seed as your hashmap bench
    let keys: Box<[[u8; 16]; N]> = {
        let mut v = vec![[0u8; 16]; N];
        for k in v.iter_mut() { *k = rng.next_key(); }
        v.into_boxed_slice().try_into().unwrap()
    };
    let value = [0xabu8; 32];

    println!("=== Hashbrown Insert Benchmark: {} keys, {} runs ===\n", N, RUNS);
    println!("{:<6} {:>10} {:>10} {:>10} {:>10} {:>10} {:>14} {:>16}",
        "Run", "Min(cyc)", "Median", "Mean", "P99", "Max", "Total(Mcyc)", "Throughput(M/s)");
    println!("{}", "-".repeat(90));

    let mut all_wall: Vec<u64> = Vec::with_capacity(RUNS);

    for run in 1..=RUNS {
        let (mut samples, wall) = run_once(&keys, value);
        let s = compute_stats(&mut samples);

        let ghz: f64 = 2.3;
        let total_ns = wall as f64 / ghz;
        let throughput = N as f64 / (total_ns / 1_000.0);

        println!("{:<6} {:>10} {:>10} {:>10.1} {:>10} {:>10} {:>14.2} {:>16.2}",
            run, s.min, s.median, s.mean, s.p99, s.max,
            wall as f64 / 1_000_000.0, throughput);

        all_wall.push(wall);
    }

    let best_wall  = *all_wall.iter().min().unwrap();
    let worst_wall = *all_wall.iter().max().unwrap();
    let avg_wall   = all_wall.iter().sum::<u64>() as f64 / RUNS as f64;
    let ghz: f64   = 2.3;

    println!("{}", "-".repeat(90));
    println!("\n=== Summary (across {} runs) ===", RUNS);
    println!("  Best  total: {:.2} M cycles  ({:.2} M inserts/sec)",
        best_wall as f64 / 1e6,
        N as f64 / (best_wall as f64 / ghz / 1_000.0));
    println!("  Worst total: {:.2} M cycles", worst_wall as f64 / 1e6);
    println!("  Avg   total: {:.2} M cycles", avg_wall / 1e6);
}

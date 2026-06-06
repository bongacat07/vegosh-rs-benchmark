mod hashmap;

const N: usize = 1_000_000;
const RUNS: usize = 5;
const HIT_RATIO: f64 = 0.8; // 80% hits, 20% misses

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

struct Xorshift128 { s: [u64; 2] }

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
    min: u64, max: u64, median: u64, p99: u64, mean: f64,
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

fn main() {
    // --- Step 1: generate inserted keys (same seed as insert bench) ---
    let mut rng = Xorshift128::new(0x1234567890abcdef);
    let inserted_keys: Vec<[u8; 16]> = (0..N).map(|_| rng.next_key()).collect();
    let value = [0xabu8; 32];

    // --- Step 2: populate the table once ---
    hashmap::init();
    for k in inserted_keys.iter() {
        hashmap::insert(k, &value, 32);
    }
    println!("Table populated: {} entries\n", hashmap::size());

    // --- Step 3: build lookup keys ---
    // 80% from inserted_keys (hits), 20% fresh random keys (misses)
    let hit_count  = (N as f64 * HIT_RATIO) as usize; // 800_000
    let miss_count = N - hit_count;                    // 200_000

    // hits: sample from inserted keys
    let mut lookup_keys: Vec<[u8; 16]> = Vec::with_capacity(N);
    let mut rng2 = Xorshift128::new(0xdeadbeef12345678); // different seed for sampling
    for _ in 0..hit_count {
        let idx = (rng2.next_u64() as usize) % N;
        lookup_keys.push(inserted_keys[idx]);
    }
    // misses: fresh keys guaranteed not in table (different seed)
    let mut rng3 = Xorshift128::new(0xfeedface87654321);
    for _ in 0..miss_count {
        lookup_keys.push(rng3.next_key());
    }
    // shuffle so hits/misses are interleaved, not batched
    // simple Fisher-Yates
    for i in (1..N).rev() {
        let j = (rng2.next_u64() as usize) % (i + 1);
        lookup_keys.swap(i, j);
    }

    println!("Lookup mix: {} hits ({:.0}%) + {} misses ({:.0}%)\n",
        hit_count, HIT_RATIO * 100.0, miss_count, (1.0 - HIT_RATIO) * 100.0);

    // --- Step 4: benchmark ---
    println!("=== Get Benchmark (80/20 hit/miss): {} lookups, {} runs ===\n", N, RUNS);
    println!("{:<6} {:>10} {:>10} {:>10} {:>10} {:>10} {:>14} {:>16}",
        "Run", "Min(cyc)", "Median", "Mean", "P99", "Max", "Total(Mcyc)", "Throughput(M/s)");
    println!("{}", "-".repeat(90));

    let mut all_wall: Vec<u64> = Vec::with_capacity(RUNS);
    let mut out = [0u8; 32];
    let mut vlen: u8 = 0;
    let mut found = 0usize; // sanity check

    for run in 1..=RUNS {
        let mut per_get: Vec<u64> = Vec::with_capacity(N);
        found = 0;

        let wall_start = rdtsc_start();
        for key in lookup_keys.iter() {
            let t0 = rdtsc_start();
            let r = hashmap::get(key, &mut out, &mut vlen);
            let t1 = rdtsc_stop();
            per_get.push(t1 - t0);
            if r == 0 { found += 1; }
        }
        let wall_end = rdtsc_stop();
        let wall = wall_end - wall_start;

        let s = compute_stats(&mut per_get);
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
    println!("  Best  total: {:.2} M cycles  ({:.2} M lookups/sec)",
        best_wall as f64 / 1e6,
        N as f64 / (best_wall as f64 / ghz / 1_000.0));
    println!("  Worst total: {:.2} M cycles", worst_wall as f64 / 1e6);
    println!("  Avg   total: {:.2} M cycles", avg_wall / 1e6);
    println!("\n  Sanity check: {} hits found out of {} lookups (expected ~{})",
        found, N, hit_count);
}

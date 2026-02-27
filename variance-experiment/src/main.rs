use sha2::{Digest, Sha256};
use std::time::Instant;

fn run_sha256(reps: u32) -> Vec<u64> {
    let buffer = vec![0xABu8; 4096];
    let mut durations = Vec::with_capacity(reps as usize);
    for _ in 0..reps {
        let start = Instant::now();
        let mut hasher = Sha256::new();
        for _ in 0..100_000 {
            hasher.update(&buffer);
        }
        let _ = hasher.finalize();
        durations.push(start.elapsed().as_nanos() as u64);
    }
    durations
}

fn run_sort(reps: u32) -> Vec<u64> {
    let mut durations = Vec::with_capacity(reps as usize);
    for _ in 0..reps {
        // Deterministic LCG PRNG, fixed seed
        let mut state: u64 = 0xDEAD_BEEF_CAFE_BABEu64;
        let mut arr: Vec<u64> = (0..1_000_000)
            .map(|_| {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                state
            })
            .collect();
        let start = Instant::now();
        arr.sort_unstable();
        durations.push(start.elapsed().as_nanos() as u64);
    }
    durations
}

fn run_matrix(reps: u32) -> Vec<u64> {
    const N: usize = 512;
    // Fixed deterministic matrices
    let a: Vec<f64> = (0..N * N).map(|i| (i as f64) * 0.001).collect();
    let b: Vec<f64> = (0..N * N).map(|i| (i as f64) * 0.001 + 1.0).collect();
    let mut c = vec![0.0f64; N * N];
    let mut durations = Vec::with_capacity(reps as usize);
    for _ in 0..reps {
        c.iter_mut().for_each(|x| *x = 0.0);
        let start = Instant::now();
        for i in 0..N {
            for k in 0..N {
                let aik = a[i * N + k];
                for j in 0..N {
                    c[i * N + j] += aik * b[k * N + j];
                }
            }
        }
        durations.push(start.elapsed().as_nanos() as u64);
        // Prevent optimization
        let _ = c[0];
    }
    durations
}

fn run_noop(reps: u32) -> Vec<u64> {
    let mut durations = Vec::with_capacity(reps as usize);
    for _ in 0..reps {
        let start = Instant::now();
        std::hint::black_box(());
        durations.push(start.elapsed().as_nanos() as u64);
    }
    durations
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut workload = String::from("sha256");
    let mut reps: u32 = 30;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--workload" => {
                i += 1;
                workload = args[i].clone();
            }
            "--reps" => {
                i += 1;
                reps = args[i].parse().expect("reps must be a positive integer");
            }
            _ => {}
        }
        i += 1;
    }

    let durations = match workload.as_str() {
        "sha256" => run_sha256(reps),
        "sort" => run_sort(reps),
        "matrix" => run_matrix(reps),
        "noop" => run_noop(reps),
        other => {
            eprintln!("Unknown workload: {other}. Valid: sha256, sort, matrix, noop");
            std::process::exit(1);
        }
    };

    // CSV output: workload,rep_index,duration_ns
    for (idx, ns) in durations.iter().enumerate() {
        println!("{workload},{idx},{ns}");
    }
}

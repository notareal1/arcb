//! Benchmarks: throughput + compression ratio vs gzip/bzip2/4-bit raw.

use ARCB::{ArcbEncoder, CompressOptions, decode_block};
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use rand::Rng;
use std::io::Write;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn random_digits(n: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..n).map(|_| rng.gen_range(0..10)).collect()
}

fn all_same(n: usize, d: u8) -> Vec<u8> {
    vec![d; n]
}

fn gzip_compress(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

fn bzip2_compress(data: &[u8]) -> Vec<u8> {
    let mut enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::best());
    enc.write_all(data).unwrap();
    enc.finish().unwrap()
}

fn pack_4bit(digits: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(digits.len().div_ceil(2));
    for chunk in digits.chunks(2) {
        let hi = chunk[0] << 4;
        let lo = if chunk.len() > 1 { chunk[1] } else { 0 };
        out.push(hi | lo);
    }
    out
}

/// Encode digits in chunks of max 65535 (superblock limit), return concatenated blocks.
fn encode_all_blocks(digits: &[u8], compress_small: bool) -> Vec<u8> {
    let opts = CompressOptions::new().with_compress_small(compress_small);
    let mut all = Vec::new();
    for chunk in digits.chunks(65535) {
        let mut enc = ArcbEncoder::with_options(opts.clone());
        for &d in chunk {
            enc.push_digit(d);
        }
        all.extend_from_slice(&enc.encode_block());
    }
    all
}

fn bench_roundtrip(c: &mut Criterion, name: &str, digits: &[u8], compress_small: bool) {
    let opts = CompressOptions::new().with_compress_small(compress_small);
    let label = if compress_small { "adaptive" } else { "raw" };

    let mut group = c.benchmark_group(format!("encode/{name}_{label}"));
    group.throughput(Throughput::Bytes(digits.len() as u64));
    group.bench_function("arcb", |b| {
        b.iter(|| {
            black_box(encode_all_blocks(digits, compress_small))
        })
    });
    group.finish();

    // Pre-compress for decode bench (just first block)
    let compressed = {
        let mut enc = ArcbEncoder::with_options(opts.clone());
        for &d in &digits[..digits.len().min(65535)] {
            enc.push_digit(d);
        }
        enc.encode_block()
    };

    let mut group = c.benchmark_group(format!("decode/{name}_{label}"));
    group.throughput(Throughput::Bytes(digits.len().min(65535) as u64));
    group.bench_function("arcb", |b| {
        b.iter(|| black_box(decode_block(black_box(&compressed)).unwrap()))
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Ratio comparison: ARCB vs gzip vs bzip2 vs 4-bit pack
// ---------------------------------------------------------------------------

fn bench_compression_ratios(c: &mut Criterion) {
    let n = 100_000;
    let digits_random = random_digits(n);
    let digits_zeros = all_same(n, 0);
    let digits_eights = all_same(n, 8);
    let digits_nines = all_same(n, 9);

    // ARCB encode
    let arcb_random_raw = encode_all_blocks(&digits_random, false);
    let arcb_zeros_raw = encode_all_blocks(&digits_zeros, false);
    let arcb_eights_raw = encode_all_blocks(&digits_eights, false);
    let arcb_nines_raw = encode_all_blocks(&digits_nines, false);
    let arcb_random_adaptive = encode_all_blocks(&digits_random, true);

    // gzip / bzip2 on ASCII
    let ascii_random: Vec<u8> = digits_random.iter().map(|&d| b'0' + d).collect();
    let ascii_zeros: Vec<u8> = digits_zeros.iter().map(|&d| b'0' + d).collect();
    let ascii_eights: Vec<u8> = digits_eights.iter().map(|&d| b'0' + d).collect();
    let ascii_nines: Vec<u8> = digits_nines.iter().map(|&d| b'0' + d).collect();

    let gzip_random = gzip_compress(&ascii_random);
    let gzip_zeros = gzip_compress(&ascii_zeros);
    let gzip_eights = gzip_compress(&ascii_eights);
    let gzip_nines = gzip_compress(&ascii_nines);

    let bz2_random = bzip2_compress(&ascii_random);
    let bz2_zeros = bzip2_compress(&ascii_zeros);
    let bz2_eights = bzip2_compress(&ascii_eights);
    let bz2_nines = bzip2_compress(&ascii_nines);

    // 4-bit packed
    let packed_random = pack_4bit(&digits_random);
    let packed_zeros = pack_4bit(&digits_zeros);
    let packed_eights = pack_4bit(&digits_eights);
    let packed_nines = pack_4bit(&digits_nines);

    let entropy = 10f64.log2();

    println!("\n=== Compression Ratio Comparison ({} digits) ===\n", n);
    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10}",
        "Method", "Random", "All-0", "All-8", "All-9"
    );
    println!("{:-<60}", "");

    let bpd = |bytes: usize| (bytes as f64 * 8.0) / n as f64;

    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10}",
        "ARCB (raw Small)",
        format!("{:.3}", bpd(arcb_random_raw.len())),
        format!("{:.3}", bpd(arcb_zeros_raw.len())),
        format!("{:.3}", bpd(arcb_eights_raw.len())),
        format!("{:.3}", bpd(arcb_nines_raw.len()))
    );
    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10}",
        "ARCB (adaptive)",
        format!("{:.3}", bpd(arcb_random_adaptive.len())),
        "-", "-", "-"
    );
    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10}",
        "gzip -9",
        format!("{:.3}", bpd(gzip_random.len())),
        format!("{:.3}", bpd(gzip_zeros.len())),
        format!("{:.3}", bpd(gzip_eights.len())),
        format!("{:.3}", bpd(gzip_nines.len()))
    );
    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10}",
        "bzip2 -9",
        format!("{:.3}", bpd(bz2_random.len())),
        format!("{:.3}", bpd(bz2_zeros.len())),
        format!("{:.3}", bpd(bz2_eights.len())),
        format!("{:.3}", bpd(bz2_nines.len()))
    );
    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10}",
        "4-bit packed",
        format!("{:.3}", bpd(packed_random.len())),
        format!("{:.3}", bpd(packed_zeros.len())),
        format!("{:.3}", bpd(packed_eights.len())),
        format!("{:.3}", bpd(packed_nines.len()))
    );
    println!(
        "{:<20} {:>10} {:>10} {:>10} {:>10}",
        "Shannon limit",
        format!("{:.3}", entropy),
        "-", "-", "-"
    );
    println!();

    // Criterion benchmark for encode throughput on 100k digits
    let mut group = c.benchmark_group("ratio_compare/encode_random_100k");
    group.throughput(Throughput::Bytes(n as u64));

    group.bench_function("arcb_raw", |b| {
        b.iter(|| black_box(encode_all_blocks(black_box(&digits_random), false)))
    });

    group.bench_function("arcb_adaptive", |b| {
        b.iter(|| black_box(encode_all_blocks(black_box(&digits_random), true)))
    });

    group.bench_function("pack_4bit", |b| {
        b.iter(|| black_box(pack_4bit(black_box(&digits_random))))
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Throughput benchmarks
// ---------------------------------------------------------------------------

fn criterion_benchmark(c: &mut Criterion) {
    let sizes = [1_000, 65_535];

    for &n in &sizes {
        let digits_random = random_digits(n);
        let digits_zeros = all_same(n, 0);
        let digits_eights = all_same(n, 8);
        let digits_nines = all_same(n, 9);

        bench_roundtrip(c, &format!("random_{n}"), &digits_random, false);
        bench_roundtrip(c, &format!("random_{n}"), &digits_random, true);
        bench_roundtrip(c, &format!("zeros_{n}"), &digits_zeros, false);
        bench_roundtrip(c, &format!("eights_{n}"), &digits_eights, false);
        bench_roundtrip(c, &format!("nines_{n}"), &digits_nines, false);
    }

    // Cross-tool ratio comparison (prints table + criterion bench)
    bench_compression_ratios(c);
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);

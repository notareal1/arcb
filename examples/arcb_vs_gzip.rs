use arcb::ArcbEncoder;
use rand::Rng;
use std::fs;
use std::process::Command;

fn arcb_compress(digits: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    for chunk in digits.chunks(65535) {
        let mut enc = ArcbEncoder::new();
        for &d in chunk {
            enc.push_digit(d);
        }
        result.extend_from_slice(&enc.encode_block());
    }
    result
}

fn gzip_compress(data: &[u8]) -> Vec<u8> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let id = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::SeqCst);
    let tmp_in = format!("/tmp/arcb_test_{}_{}.dat", id, seq);
    let gzip_out = format!("{}.gz", tmp_in);
    fs::write(&tmp_in, data).unwrap();
    let output = Command::new("gzip")
        .args(["-9", "-f", &tmp_in])
        .output()
        .unwrap();
    assert!(output.status.success(), "gzip failed: {:?}", output.stderr);
    let compressed = fs::read(&gzip_out).unwrap();
    let _ = fs::remove_file(&tmp_in);
    let _ = fs::remove_file(&gzip_out);
    compressed
}

fn test_scenario(name: &str, digits: &[u8]) {
    let n = digits.len();

    let arcb_compressed = arcb_compress(digits);
    let arcb_bits = arcb_compressed.len() * 8;
    let arcb_bpd = arcb_bits as f64 / n as f64;

    let ascii: Vec<u8> = digits.iter().map(|&d| b'0' + d).collect();
    let gzip_compressed = gzip_compress(&ascii);
    let gzip_bits = gzip_compressed.len() * 8;
    let gzip_bpd = gzip_bits as f64 / n as f64;

    let mut packed = Vec::new();
    for chunk in digits.chunks(2) {
        let hi = chunk[0] << 4;
        let lo = if chunk.len() > 1 { chunk[1] } else { 0 };
        packed.push(hi | lo);
    }
    let gzip_packed_compressed = gzip_compress(&packed);
    let gzip_packed_bits = gzip_packed_compressed.len() * 8;
    let gzip_packed_bpd = gzip_packed_bits as f64 / n as f64;

    let entropy = 10f64.log2();

    println!("=== {} ({} digits) ===", name, n);
    println!("  Entropy limit:      {:.3} bits/digit", entropy);
    println!(
        "  ARCB:               {:.3} bits/digit  ({} bytes)",
        arcb_bpd,
        arcb_compressed.len()
    );
    println!(
        "  Gzip (ASCII):       {:.3} bits/digit  ({} bytes)",
        gzip_bpd,
        gzip_compressed.len()
    );
    println!(
        "  Gzip (4-bit pack):  {:.3} bits/digit  ({} bytes)",
        gzip_packed_bpd,
        gzip_packed_compressed.len()
    );

    if arcb_bpd < gzip_bpd {
        println!(
            "  -> ARCB beats Gzip (ASCII) by {:.1}%",
            (1.0 - arcb_bpd / gzip_bpd) * 100.0
        );
    } else {
        println!(
            "  -> Gzip (ASCII) beats ARCB by {:.1}%",
            (1.0 - gzip_bpd / arcb_bpd) * 100.0
        );
    }
    println!();
}

fn main() {
    let mut rng = rand::thread_rng();
    let n = 100_000;

    println!("========================================");
    println!("  ARCB vs Gzip -- 100K digits");
    println!("========================================\n");

    let random: Vec<u8> = (0..n).map(|_| rng.gen_range(0..10)).collect();
    test_scenario("Uniform random 0-9", &random);

    let zeros = vec![0u8; n];
    test_scenario("All zeros", &zeros);

    let eights = vec![8u8; n];
    test_scenario("All eights", &eights);

    let nines = vec![9u8; n];
    test_scenario("All nines", &nines);

    let biased: Vec<u8> = (0..n)
        .map(|_| {
            let r = rng.gen_range(0..10);
            if r < 7 {
                rng.gen_range(0..8)
            } else {
                if rng.gen_bool(0.5) { 8 } else { 9 }
            }
        })
        .collect();
    test_scenario("Biased 70% small / 30% large", &biased);

    let phone: Vec<u8> = (0..n)
        .map(|i| {
            let pattern = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
            pattern[i % 10]
        })
        .collect();
    test_scenario("Phone-like pattern (repeating)", &phone);

    let repeating: Vec<u8> = (0..n).map(|i| (i % 10) as u8).collect();
    test_scenario("Repeating 0-9 sequence", &repeating);

    let mut runs = Vec::new();
    while runs.len() < n {
        let digit = rng.gen_range(0..10);
        let run_len = rng.gen_range(1..20).min(n - runs.len());
        for _ in 0..run_len {
            runs.push(digit);
        }
    }
    test_scenario("Random runs (realistic)", &runs);

    let fives = vec![5u8; n];
    test_scenario("All fives", &fives);

    let alt: Vec<u8> = (0..n).map(|i| if i % 2 == 0 { 8 } else { 9 }).collect();
    test_scenario("Alternating 8/9", &alt);
}

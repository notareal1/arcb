use std::io::{Read, Write};
use std::time::Instant;
use rand::Rng;

fn main() {
    let n = 65_535usize;
    let entropy = 10f64.log2();

    let mut rng = rand::thread_rng();
    let digits: Vec<u8> = (0..n).map(|_| rng.gen_range(0..10)).collect();
    let ascii: Vec<u8> = digits.iter().map(|&d| b'0' + d).collect();

    println!("=== Benchmark: {} random decimal digits ===\n", n);

    // --- ARCB (raw Small) ---
    let start = Instant::now();
    let mut enc = arcb::ArcbEncoder::new();
    for &d in &digits { enc.push_digit(d); }
    let arcb_compressed = enc.encode_block();
    let arcb_enc_ms = start.elapsed().as_secs_f64() * 1000.0;

    let start = Instant::now();
    let arcb_decoded = arcb::decode_block(&arcb_compressed).unwrap();
    let arcb_dec_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(arcb_decoded, digits);

    let arcb_bpd = (arcb_compressed.len() as f64 * 8.0) / n as f64;
    let arcb_enc_mbps = (n as f64 / 1_000_000.0) / (arcb_enc_ms / 1000.0);
    let arcb_dec_mbps = (n as f64 / 1_000_000.0) / (arcb_dec_ms / 1000.0);

    // --- ARCB (adaptive Small) ---
    let start = Instant::now();
    let opts = arcb::CompressOptions::new().with_compress_small(true);
    let mut enc = arcb::ArcbEncoder::with_options(opts);
    for &d in &digits { enc.push_digit(d); }
    let arcb_adapt_compressed = enc.encode_block();
    let arcb_adapt_enc_ms = start.elapsed().as_secs_f64() * 1000.0;
    let arcb_adapt_bpd = (arcb_adapt_compressed.len() as f64 * 8.0) / n as f64;

    // --- gzip -9 ---
    let start = Instant::now();
    let mut gz_enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    gz_enc.write_all(&ascii).unwrap();
    let gz_compressed = gz_enc.finish().unwrap();
    let gz_enc_ms = start.elapsed().as_secs_f64() * 1000.0;

    let start = Instant::now();
    let gz_dec = flate2::read::GzDecoder::new(&gz_compressed[..]);
    let mut gz_decoded = Vec::new();
    gz_dec.take(n as u64).read_to_end(&mut gz_decoded).unwrap();
    let gz_dec_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(gz_decoded, ascii);

    let gz_bpd = (gz_compressed.len() as f64 * 8.0) / n as f64;
    let gz_enc_mbps = (n as f64 / 1_000_000.0) / (gz_enc_ms / 1000.0);
    let gz_dec_mbps = (n as f64 / 1_000_000.0) / (gz_dec_ms / 1000.0);

    // --- bzip2 -9 ---
    let start = Instant::now();
    let mut bz_enc = bzip2::write::BzEncoder::new(Vec::new(), bzip2::Compression::best());
    bz_enc.write_all(&ascii).unwrap();
    let bz2_compressed = bz_enc.finish().unwrap();
    let bz2_enc_ms = start.elapsed().as_secs_f64() * 1000.0;

    let start = Instant::now();
    let bz_dec = bzip2::read::BzDecoder::new(&bz2_compressed[..]);
    let mut bz2_decoded = Vec::new();
    bz_dec.take(n as u64).read_to_end(&mut bz2_decoded).unwrap();
    let bz2_dec_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(bz2_decoded, ascii);

    let bz2_bpd = (bz2_compressed.len() as f64 * 8.0) / n as f64;
    let bz2_enc_mbps = (n as f64 / 1_000_000.0) / (bz2_enc_ms / 1000.0);
    let bz2_dec_mbps = (n as f64 / 1_000_000.0) / (bz2_dec_ms / 1000.0);

    // --- Results ---
    println!(
        "{:<22} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Method", "Bits/digit", "Enc(ms)", "Dec(ms)", "Enc(MB/s)", "Dec(MB/s)"
    );
    println!("{:-<80}", "");

    println!(
        "{:<22} {:>10.3} {:>10.1} {:>10.1} {:>10.1} {:>10.1}",
        "ARCB (raw Small)", arcb_bpd, arcb_enc_ms, arcb_dec_ms, arcb_enc_mbps, arcb_dec_mbps
    );
    println!(
        "{:<22} {:>10.3} {:>10.1} {:>10} {:>10} {:>10}",
        "ARCB (adaptive)", arcb_adapt_bpd, arcb_adapt_enc_ms, "-", "-", "-"
    );
    println!(
        "{:<22} {:>10.3} {:>10.1} {:>10.1} {:>10.1} {:>10.1}",
        "gzip -9", gz_bpd, gz_enc_ms, gz_dec_ms, gz_enc_mbps, gz_dec_mbps
    );
    println!(
        "{:<22} {:>10.3} {:>10.1} {:>10.1} {:>10.1} {:>10.1}",
        "bzip2 -9", bz2_bpd, bz2_enc_ms, bz2_dec_ms, bz2_enc_mbps, bz2_dec_mbps
    );

    println!("\nShannon limit: {:.3} bits/digit", entropy);
    println!("Original (ASCII): {} bytes", n);
    println!("ARCB (raw):       {} bytes", arcb_compressed.len());
    println!("ARCB (adaptive):  {} bytes", arcb_adapt_compressed.len());
    println!("gzip -9:          {} bytes", gz_compressed.len());
    println!("bzip2 -9:         {} bytes", bz2_compressed.len());
}

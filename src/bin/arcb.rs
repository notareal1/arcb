//! ARCB — Decimal digit string compressor (CLI).
//!
//! Compresses and decompresses files containing decimal digit strings.
//! Uses adaptive range coding for near-entropy-limit compression.

use clap::{Parser, Subcommand};
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "arcb", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compress a digit string file to .arcb format
    Compress {
        /// Input file (or stdin if not provided)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output file (or stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Use adaptive Small-value compression
        #[arg(long, default_value_t = true)]
        small_compress: bool,

        /// Append CRC-32 checksum
        #[arg(long, default_value_t = true)]
        checksum: bool,
    },

    /// Decompress an .arcb file to digit string
    Decompress {
        /// Input file (or stdin if not provided)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Output file (or stdout if not provided)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show compression statistics
    Stats {
        /// Input file (or stdin if not provided)
        #[arg(short, long)]
        input: Option<PathBuf>,

        /// Use adaptive Small-value compression
        #[arg(long, default_value_t = true)]
        small_compress: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compress {
            input,
            output,
            small_compress,
            checksum,
        } => cmd_compress(input, output, small_compress, checksum),
        Commands::Decompress { input, output } => cmd_decompress(input, output),
        Commands::Stats { input, small_compress } => cmd_stats(input, small_compress),
    }
}

fn read_input(input: Option<PathBuf>) -> io::Result<String> {
    match input {
        Some(path) => fs::read_to_string(&path),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(buf)
        }
    }
}

fn write_output(output: Option<PathBuf>, data: &[u8]) -> io::Result<()> {
    match output {
        Some(path) => fs::write(&path, data),
        None => {
            io::stdout().write_all(data)?;
            Ok(())
        }
    }
}

fn clean_digits(s: &str) -> Result<String, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("empty input".to_string());
    }
    if !cleaned.chars().all(|c| c.is_ascii_digit()) {
        return Err("input must contain only digits 0-9".to_string());
    }
    Ok(cleaned)
}

fn cmd_compress(input: Option<PathBuf>, output: Option<PathBuf>, small_compress: bool, checksum: bool) {
    let raw = match read_input(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading input: {e}");
            std::process::exit(1);
        }
    };

    let digits = match clean_digits(&raw) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let n = digits.len();
    let start = Instant::now();

    let compressed = {
        use arcb::ArcbEncoder;
        if small_compress {
            let opts = arcb::CompressOptions::new().with_compress_small(true);
            let mut enc = ArcbEncoder::with_options(opts);
            for ch in digits.chars() {
                enc.push_digit(ch.to_digit(10).unwrap() as u8);
            }
            let block = enc.encode_block();
            arcb::encode_block_to_file_format(&block, checksum)
        } else if checksum {
            arcb::encode_to_binary_with_checksum(&digits).unwrap()
        } else {
            arcb::encode_to_binary(&digits).unwrap()
        }
    };

    let elapsed = start.elapsed();

    if let Err(e) = write_output(output, &compressed) {
        eprintln!("Error writing output: {e}");
        std::process::exit(1);
    }

    let ratio = compressed.len() as f64 / n as f64;
    let bpd = (compressed.len() as f64 * 8.0) / n as f64;
    let entropy = 10f64.log2();

    eprintln!(
        "Compressed {n} digits -> {} bytes ({ratio:.3}x, {bpd:.3} bits/digit)",
        compressed.len()
    );
    eprintln!(
        "Entropy limit: {entropy:.3} bits/digit ({:.1}% of entropy)",
        bpd / entropy * 100.0
    );
    eprintln!("Time: {elapsed:.2?}");
}

fn cmd_decompress(input: Option<PathBuf>, output: Option<PathBuf>) {
    let data: Vec<u8> = match input {
        Some(path) => match fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error reading input: {e}");
                std::process::exit(1);
            }
        },
        None => {
            let mut buf = Vec::new();
            if let Err(e) = io::stdin().read_to_end(&mut buf) {
                eprintln!("Error reading stdin: {e}");
                std::process::exit(1);
            }
            buf
        }
    };

    match arcb::decode_from_binary(&data) {
        Ok(digits) => {
            if let Err(e) = write_output(output, digits.as_bytes()) {
                eprintln!("Error writing output: {e}");
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Decompression error: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_stats(input: Option<PathBuf>, small_compress: bool) {
    let raw = match read_input(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading input: {e}");
            std::process::exit(1);
        }
    };

    let digits = match clean_digits(&raw) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    let n = digits.len();

    let compressed_raw = arcb::encode_to_binary(&digits).unwrap();
    let opts = arcb::CompressOptions::new().with_compress_small(small_compress);

    use arcb::ArcbEncoder;
    let mut enc = ArcbEncoder::with_options(opts);
    for ch in digits.chars() {
        enc.push_digit(ch.to_digit(10).unwrap() as u8);
    }
    let compressed_adaptive = enc.encode_block();

    let decoded = arcb::decode_block(&compressed_adaptive).unwrap();
    let digit_bytes: Vec<u8> = digits
        .chars()
        .map(|c| c.to_digit(10).unwrap() as u8)
        .collect();
    let roundtrip_ok = decoded == digit_bytes;

    let file_compressed = arcb::encode_to_binary_with_checksum(&digits).unwrap();
    let file_decoded = arcb::decode_from_binary_checked(&file_compressed).unwrap();
    let file_roundtrip_ok = file_decoded == digits;

    let bpd_raw = (compressed_raw.len() as f64 * 8.0) / n as f64;
    let bpd_adaptive = (compressed_adaptive.len() as f64 * 8.0) / n as f64;
    let entropy = 10f64.log2();

    println!("=== ARCB Compression Statistics ===");
    println!("Input digits:   {n}");
    println!("Input bytes:    {n} (ASCII)");
    println!();
    println!("--- RAW Small (3 bits/value) ---");
    println!("Compressed:     {} bytes", compressed_raw.len());
    println!("Ratio:          {:.3}x", compressed_raw.len() as f64 / n as f64);
    println!("Bits/digit:     {bpd_raw:.3}");
    println!("Entropy:        {:.1}%", bpd_raw / entropy * 100.0);
    println!();
    println!("--- ADAPTIVE Small ---");
    println!("Compressed:     {} bytes", compressed_adaptive.len());
    println!("Ratio:          {:.3}x", compressed_adaptive.len() as f64 / n as f64);
    println!("Bits/digit:     {bpd_adaptive:.3}");
    println!("Entropy:        {:.1}%", bpd_adaptive / entropy * 100.0);
    println!("Roundtrip:      {}", if roundtrip_ok { "PASS" } else { "FAIL" });
    println!();
    println!("--- FILE FORMAT (with CRC-32) ---");
    println!("Compressed:     {} bytes", file_compressed.len());
    println!("CRC verified:   {}", if file_roundtrip_ok { "PASS" } else { "FAIL" });
    println!();
    println!("Shannon limit:  {entropy:.3} bits/digit");
    println!(
        "vs 4-bit/pack:   {:.1}%",
        (1.0 - bpd_adaptive / 4.0) * 100.0
    );
    println!(
        "vs ASCII:        {:.1}%",
        (1.0 - bpd_adaptive / 8.0) * 100.0
    );
}

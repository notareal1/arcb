use arcb::ArcbEncoder;

fn main() {
    let input = "13829482487248734";
    
    println!("Input:  {}", input);
    println!("Length: {} digits", input.len());

    let digits: Vec<u8> = input
        .chars()
        .map(|c| c.to_digit(10).unwrap() as u8)
        .collect();

    let mut encoder = ArcbEncoder::new();
    for &d in &digits {
        encoder.push_digit(d);
    }
    let compressed = encoder.encode_block();

    let input_bits = input.len() * 8; // as ASCII
    let compressed_bits = compressed.len() * 8;
    let theoretical_min = (input.len() as f64) * 10f64.log2(); // ~3.322 bits/digit

    println!("Compressed size: {} bytes ({} bits)", compressed.len(), compressed_bits);
    println!("Original (ASCII): {} bytes ({} bits)", input.len(), input_bits);
    println!("Ratio: {:.3}x", compressed.len() as f64 / input.len() as f64);
    println!("Bits/digit: {:.3}", compressed_bits as f64 / input.len() as f64);
    println!("Theoretical min: {:.3} bits/digit", theoretical_min / input.len() as f64);

    // Verify round-trip
    let decompressed = arcb::decode_block(&compressed).unwrap();
    let decompressed_str: String = decompressed.iter().map(|d| (b'0' + d) as char).collect();
    println!("Round-trip OK: {}", if decompressed_str == input { "PASS" } else { "FAIL" });
    if decompressed_str != input {
        println!("  Expected: {}", input);
        println!("  Got:      {}", decompressed_str);
    }
}

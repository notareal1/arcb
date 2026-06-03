# ARCB Theory of Operation

## 0. Context: ARCB vs FSE/ANS

ARCB is **not** a new entropy coding algorithm. It is a **domain-specific
compression scheme** for decimal digit strings.

| Aspect | FSE / ANS | ARCB |
|---|---|---|
| Type | General entropy coder | Domain-specific compressor |
| Input | Any byte sequence | Decimal digits only |
| Core technique | Table-based state machine | Two-group decomposition + adaptive range coding |
| Speed | Very fast (table lookup) | Slower (range coding) |
| Use case | Production (zstd, etc.) | Educational / learning |

**FSE** (Finite State Entropy) and **ANS** (Asymmetric Numerical Systems) are
state-of-the-art entropy coding methods used in production compressors like
zstd. They use lookup tables for fast encode/decode and achieve near-optimal
compression on any data.

**ARCB** uses simpler range coding (via the `constriction` crate) combined with
a two-group decomposition specific to decimal alphabets (digits 0-9 split into
groups of 8 and 2). This decomposition only makes sense for base-10 digits and
cannot be generalised.

**When to use ARCB:** Learning, experimentation, or when you specifically need
to compress decimal digit strings and want to understand entropy coding.

**When NOT to use ARCB:** For production workloads, general data compression,
or when speed matters. Use zstd/gzip/brotli instead.

## 1. Problem Statement

Given a string of N decimal digits d1 d2 ... dN where each di in {0,1,...,9},
produce a compressed bit stream that is as close as possible to the Shannon
entropy H = log2(10) ~ 3.3219 bits per digit.

General-purpose compressors (gzip, bzip2) achieve this on patterned data via
dictionary coding (LZ77/LZ78) but approach 4 bits/digit on uniform random data
because they treat each byte (ASCII character) independently. ARCB is
domain-specific: it knows the alphabet is exactly {0,...,9} and exploits this
structure.

## 2. Two-Group Decomposition

Each digit is classified as:

* **Small**: d in {0, ..., 7} -- 8 possible values
* **Large**: d in {8, 9} -- 2 possible values

A **bit mask** M of length N records the classification:
Mi = 0 if di is Small, Mi = 1 if di is Large.

The mask itself is a binary sequence with bias (typically ~80% zeros for
uniform data), making it highly compressible with a binary entropy coder.

For Large digits, a secondary bit stream L records whether each is 8 (Lj=0)
or 9 (Lj=1).

The overall decomposition:

```
digits:  [8, 3, 9, 1, 0, 2, 7, 4, 6, 5]
mask:    [1, 0, 1, 0, 0, 0, 0, 0, 0, 0]   (1 = Large)
large:   [0, 1]                              (0=8, 1=9)
small:   [3, 1, 0, 2, 7, 4, 6, 5]           (values 0-7)
```

Three independent compressed streams are produced:

1. **Mask stream** -- compressed via adaptive binary range coding
2. **Large bits stream** -- compressed via adaptive binary range coding
3. **Small values stream** -- either raw 3 bits/value or adaptive 8-symbol range coding

## 3. Range Coding

ARCB uses a **queue-based (FIFO) range coder** from the `constriction` crate.

### 3.1 Binary Model (for Mask and Large Bits)

An adaptive binary model maintains counts [c0, c1], initialized to [1, 1].
P(0) = c0 / (c0 + c1), P(1) = c1 / (c0 + c1), in 16-bit fixed-point
arithmetic.

After each symbol, the corresponding count is incremented. When the total
exceeds 2^16, both counts are halved (with rounding), preserving adaptivity to
local statistics.

### 3.2 8-Symbol Model (for Small Values)

Extends the same idea to 8 symbols: counts [c0, ..., c7], initialized to [1; 8].
P(k) = ck / sum(ci). Each encode/decode step updates one count and optionally
halves all counts when the total exceeds 2^16.

Fixed-point formula (16-bit precision):

```
left_cumulative(k) = (sum(c[0..k]) << 16) / total
probability(k)     = max(1, (c[k] << 16) / total)
```

## 4. Superblock Structure

Each superblock holds up to 65 535 digits (2^16 - 1) and is compressed
independently. The 11-byte header allows the decoder to locate each stream:

```
[flags:1][n:2][small_count:2][mask_len:2][large_len:2][small_len:2]
[  mask_compressed  ][  large_compressed  ][  small_data  ]
```

## 5. Entropy Analysis

For uniform random digits (each with probability 0.1):

**Simple 4-bit encoding**: 4.000 bits/digit (inefficient)

**Entropy of single digit**: H = log2(10) = 3.322 bits/digit

**ARCB overhead components**:
- Mask entropy: H2(0.2) = 0.722 bits/digit (binomial, P(Large)=0.2)
- Large bit entropy: 1 x 0.2 = 0.200 bits/digit (uniform 8/9)
- Small value entropy: log2(8) x 0.8 = 2.400 bits/digit
- **Total**: 3.322 bits/digit (matches entropy!)

In practice, range coding introduces small overhead (~0.01 bits/symbol for
adaptive models), giving ~3.33 bits/digit on random data.

**When Small compression is enabled**, the 8-symbol model further reduces
the Small stream toward its entropy limit. For non-uniform distributions
(e.g., 70% digit 0), the adaptive model achieves close to
-0.7*log2(0.7) - 0.3*log2(0.14)*8 ~ 2.1 bits for the Small portion.

## 6. Comparison with General-Purpose Compressors

| Algorithm | Mechanism | Random digit data | Patterned digit data |
|---|---|---|---|
| ARCB | Adaptive range coding | **~3.33 bpd** | ~0.5-2 bpd |
| gzip (LZ77 + Huffman) | Dictionary + Huffman | ~3.9 bpd | **~0.01-0.5 bpd** |
| bzip2 (BWT + Huffman) | Transform + Huffman | ~3.8 bpd | **~0.01-0.3 bpd** |

ARCB wins on random/semi-random data by exploiting the known {0,...,9} alphabet.
General-purpose compressors win on repetitive data through LZ77/BWT
dictionary matching. For mixed workloads, ARCB's adaptive model gracefully
degrades toward 4 bits/digit on pattern-rich data.

## 7. Integrity Verification

### 7.1 Block-Level: Mask/Large Consistency

After decoding the mask, the decoder counts the number of 1-bits (Large
positions) and verifies it matches `large_count = n - small_count` from the
header. A mismatch indicates corruption in the mask or large-bits stream.

### 7.2 File-Level: CRC-32

When `encode_to_binary_with_checksum` is used, a CRC-32 checksum (IEEE 802.3
polynomial 0xEDB88320) is computed over the block payload and stored
immediately after the version byte. The decoder validates this checksum
before decompression, detecting any single-bit or multi-bit file corruption.

//! Adaptive entropy models for constriction range coding.

use constriction::BitArray;
use constriction::stream::model::{DecoderModel, EncoderModel, EntropyModel};
use core::borrow::Borrow;
use std::num::NonZeroU32;

// ---------------------------------------------------------------------------
// AdaptiveBinaryModel (2 symbols: 0 / 1)
// ---------------------------------------------------------------------------

/// Adaptive binary entropy model with 16-bit precision.
///
/// Tracks counts for two symbols (0 and 1) and halves them when the
/// total exceeds `2^16`, keeping the model responsive to local statistics.
#[derive(Debug, Clone)]
pub struct AdaptiveBinaryModel {
    counts: [u32; 2],
}

impl AdaptiveBinaryModel {
    const MAX_TOTAL: u32 = 1 << 16;

    pub fn new() -> Self {
        Self { counts: [1, 1] }
    }

    pub fn update(&mut self, symbol: usize) {
        self.counts[symbol] += 1;
        if self.counts[0] + self.counts[1] >= Self::MAX_TOTAL {
            self.counts[0] = (self.counts[0] + 1) >> 1;
            self.counts[1] = (self.counts[1] + 1) >> 1;
        }
    }
}

impl EntropyModel<16> for AdaptiveBinaryModel {
    type Symbol = usize;
    type Probability = u32;
}

impl EncoderModel<16> for AdaptiveBinaryModel {
    fn left_cumulative_and_probability(
        &self,
        symbol: impl Borrow<Self::Symbol>,
    ) -> Option<(Self::Probability, <Self::Probability as BitArray>::NonZero)> {
        let sym = *symbol.borrow();
        if sym > 1 {
            return None;
        }
        let total = (self.counts[0] + self.counts[1]) as u64;
        let cum0 = self.counts[0] as u64;

        let left_cumulative = if sym == 0 {
            0u32
        } else {
            ((cum0 << 16) / total) as u32
        };

        let probability = if sym == 0 {
            ((cum0 << 16) / total) as u32
        } else {
            (((self.counts[1] as u64) << 16) / total) as u32
        };

        let probability = probability.max(1);
        Some((left_cumulative, probability.try_into().ok()?))
    }
}

impl DecoderModel<16> for AdaptiveBinaryModel {
    fn quantile_function(
        &self,
        quantile: Self::Probability,
    ) -> (
        Self::Symbol,
        Self::Probability,
        <Self::Probability as BitArray>::NonZero,
    ) {
        let total = (self.counts[0] + self.counts[1]) as u64;
        let cum0 = (((self.counts[0] as u64) << 16) / total).max(1) as u32;

        if quantile < cum0 {
            (0, 0, cum0.try_into().unwrap())
        } else {
            let prob1 = (((self.counts[1] as u64) << 16) / total).max(1) as u32;
            (1, cum0, prob1.try_into().unwrap())
        }
    }
}

// ---------------------------------------------------------------------------
// AdaptiveSymbolModel (N symbols, const generic)
// ---------------------------------------------------------------------------

/// Adaptive entropy model for `N` equiprobable-at-init symbols.
///
/// Used for Small-value compression where each symbol in {0, ..., N-1}
/// represents a decimal digit. Counts start at `[1; N]` and are halved
/// together when their sum reaches `2^16`, preserving adaptivity.
#[derive(Debug, Clone)]
pub struct AdaptiveSymbolModel<const N: usize> {
    counts: [u32; N],
}

impl<const N: usize> AdaptiveSymbolModel<N> {
    const MAX_TOTAL: u32 = 1 << 16;

    /// Creates a fresh model with all counts initialised to 1.
    pub fn new() -> Self {
        Self { counts: [1; N] }
    }

    /// Update the model after encoding/decoding `symbol` (0-based index).
    pub fn update(&mut self, symbol: usize) {
        debug_assert!(symbol < N, "symbol {symbol} out of range 0..{N}");
        self.counts[symbol] += 1;
        let total: u32 = self.counts.iter().sum();
        if total >= Self::MAX_TOTAL {
            for c in &mut self.counts {
                *c = (*c + 1) >> 1;
            }
        }
    }

    /// Direct access to the count array (for testing / debugging).
    #[cfg(test)]
    pub fn counts(&self) -> &[u32; N] {
        &self.counts
    }
}

impl<const N: usize> EntropyModel<16> for AdaptiveSymbolModel<N> {
    type Symbol = usize;
    type Probability = u32;
}

impl<const N: usize> EncoderModel<16> for AdaptiveSymbolModel<N> {
    fn left_cumulative_and_probability(
        &self,
        symbol: impl Borrow<Self::Symbol>,
    ) -> Option<(Self::Probability, <Self::Probability as BitArray>::NonZero)> {
        let sym = *symbol.borrow();
        if sym >= N {
            return None;
        }
        let total: u64 = self.counts.iter().map(|&c| c as u64).sum();

        // Left cumulative = sum of counts[0..sym]
        let left_cum: u64 = self.counts[..sym].iter().map(|&c| c as u64).sum();
        let left_cumulative = ((left_cum << 16) / total) as u32;

        let prob = (((self.counts[sym] as u64) << 16) / total).max(1) as u32;

        Some((left_cumulative, NonZeroU32::new(prob)?))
    }
}

impl<const N: usize> DecoderModel<16> for AdaptiveSymbolModel<N> {
    fn quantile_function(
        &self,
        quantile: Self::Probability,
    ) -> (
        Self::Symbol,
        Self::Probability,
        <Self::Probability as BitArray>::NonZero,
    ) {
        let total: u64 = self.counts.iter().map(|&c| c as u64).sum();

        // Build cumulative distribution in fixed-point 16
        let mut cum: u64 = 0;
        for (i, &count) in self.counts.iter().enumerate() {
            let cum_left = ((cum << 16) / total) as u32;
            cum += count as u64;
            let cum_right = ((cum << 16) / total).max((cum_left as u64) + 1) as u32;

            if quantile < cum_right {
                let prob = (((count as u64) << 16) / total).max(1) as u32;
                return (i, cum_left, NonZeroU32::new(prob).unwrap());
            }
        }

        // Floating-point rounding: clamp to last symbol
        let last_prob = (((self.counts[N - 1] as u64) << 16) / total).max(1) as u32;
        let last_cum: u64 = self.counts[..N - 1].iter().map(|&c| c as u64).sum();
        (
            N - 1,
            ((last_cum << 16) / total) as u32,
            NonZeroU32::new(last_prob).unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_basic() {
        let mut model = AdaptiveBinaryModel::new();
        let result = model.left_cumulative_and_probability(0usize);
        assert!(result.is_some());
        let (cum, _prob) = result.unwrap();
        assert_eq!(cum, 0);

        for _ in 0..100 {
            model.update(1);
        }
        let (_, prob0) = model.left_cumulative_and_probability(0usize).unwrap();
        let (_, prob1) = model.left_cumulative_and_probability(1usize).unwrap();
        assert!(prob1 > prob0, "P(1) should be > P(0)");
    }

    #[test]
    fn model_halving() {
        let mut model = AdaptiveBinaryModel::new();
        for _ in 0..((AdaptiveBinaryModel::MAX_TOTAL + 100) as usize) {
            model.update(1);
        }
        assert!(model.counts[0] + model.counts[1] < AdaptiveBinaryModel::MAX_TOTAL);
    }

    #[test]
    fn symbol_model_uniform() {
        let model = AdaptiveSymbolModel::<8>::new();
        let mut probs = [0u32; 8];
        for sym in 0..8 {
            let (_, p) = model.left_cumulative_and_probability(sym).unwrap();
            probs[sym] = p.get();
        }
        // In fixed-point 16, each prob = (1/8) * 2^16 = 8192
        for p in &probs {
            assert!(*p >= 8000 && *p <= 8400, "prob {p} out of expected range");
        }
    }

    #[test]
    fn symbol_model_skewed() {
        let mut model = AdaptiveSymbolModel::<8>::new();
        for _ in 0..200 {
            model.update(7);
        }
        let (_, prob7) = model.left_cumulative_and_probability(7).unwrap();
        let (_, prob0) = model.left_cumulative_and_probability(0).unwrap();
        assert!(prob7 > prob0, "P(7) should dominate after 200 updates");
    }

    #[test]
    fn symbol_model_symmetric_roundtrip() {
        use constriction::stream::{Decode, queue::DefaultRangeDecoder};
        use constriction::stream::Encode;

        let seq = vec![0usize, 1, 2, 3, 4, 5, 6, 7, 0, 0, 7];
        let mut enc = constriction::stream::queue::DefaultRangeEncoder::new();
        let mut model_enc = AdaptiveSymbolModel::<8>::new();
        for &sym in &seq {
            enc.encode_symbol(sym, &model_enc).unwrap();
            model_enc.update(sym);
        }
        let words: Vec<u32> = enc.into_compressed().unwrap();

        let mut dec = DefaultRangeDecoder::from_compressed(&words).unwrap();
        let mut model_dec = AdaptiveSymbolModel::<8>::new();
        let mut decoded = Vec::with_capacity(seq.len());
        for _ in 0..seq.len() {
            let sym = dec.decode_symbol(&model_dec).unwrap();
            model_dec.update(sym);
            decoded.push(sym);
        }
        assert_eq!(decoded, seq);
    }

    #[test]
    fn symbol_model_halving() {
        let mut model = AdaptiveSymbolModel::<8>::new();
        for _ in 0..((AdaptiveSymbolModel::<8>::MAX_TOTAL + 1000) as usize) {
            model.update(3);
        }
        let total: u32 = model.counts.iter().sum();
        assert!(total < AdaptiveSymbolModel::<8>::MAX_TOTAL);
    }
}

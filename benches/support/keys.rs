/// Deterministic xorshift64 generator.
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub(crate) fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `0..bound` by multiply-shift.
    #[allow(clippy::cast_possible_truncation, reason = "result is below `bound`")]
    pub(crate) fn below(&mut self, bound: usize) -> usize {
        ((u128::from(self.next()) * bound as u128) >> 64) as usize
    }

    pub(crate) fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

/// Key sequences for one size `n`, drawn from the domain `0..2n`.
pub(crate) struct Keys {
    pub(crate) n: usize,
    pub(crate) domain: usize,
    /// The `n` live keys, in insertion order.
    pub(crate) live: Vec<u32>,
    /// The live keys, reshuffled so lookups do not follow dense order.
    pub(crate) hits: Vec<u32>,
    /// The other `n` keys of the domain, never inserted.
    pub(crate) misses: Vec<u32>,
    /// The whole domain shuffled: half hits, half misses.
    pub(crate) mixed: Vec<u32>,
}

impl Keys {
    pub(crate) fn new(n: usize) -> Self {
        let domain = 2 * n;
        let mut rng = Rng::new(0x9E37_79B9_7F4A_7C15 ^ n as u64);
        let mut live: Vec<u32> = (0..u32::try_from(domain).unwrap()).collect();
        rng.shuffle(&mut live);
        let misses = live.split_off(n);
        let mut hits = live.clone();
        rng.shuffle(&mut hits);
        let mut mixed = [live.as_slice(), &misses].concat();
        rng.shuffle(&mut mixed);
        Self {
            n,
            domain,
            live,
            hits,
            misses,
            mixed,
        }
    }
}

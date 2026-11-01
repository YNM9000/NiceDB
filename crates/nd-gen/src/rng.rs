#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn derive(seed: u64, part: u64) -> Self {
        let mut r = Self(seed ^ part.wrapping_mul(0xA076_1D64_78BD_642F));
        r.bits();
        r
    }

    pub fn bits(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        let wide = u128::from(self.bits()) * u128::from(n);
        u64::try_from(wide >> 64).unwrap_or(0)
    }

    pub fn index(&mut self, len: usize) -> usize {
        usize::try_from(self.below(len as u64)).unwrap_or(0)
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.index(items.len())]
    }

    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
        let span = hi.abs_diff(lo).saturating_add(1);
        lo.wrapping_add_unsigned(self.below(span))
    }

    pub fn unit(&mut self) -> f64 {
        #[allow(clippy::cast_precision_loss)]
        let v = (self.bits() >> 11) as f64 / (1u64 << 53) as f64;
        v
    }

    pub fn chance(&mut self, p: f64) -> bool {
        p > 0.0 && self.unit() < p
    }
}

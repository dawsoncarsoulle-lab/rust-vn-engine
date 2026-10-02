//! Portable, saveable story randomness. Not suitable for cryptography.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomState {
    state: u64,
}

impl Default for RandomState {
    fn default() -> Self {
        Self::seeded(0x7275_7374_2d56_4e01)
    }
}

impl RandomState {
    pub const fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    pub fn fresh() -> Self {
        #[cfg(target_arch = "wasm32")]
        let seed = (js_sys::Date::now() * 1000.0) as u64;
        #[cfg(not(target_arch = "wasm32"))]
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|time| time.as_nanos() as u64)
            .unwrap_or(0);
        Self::seeded(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        // SplitMix64's explicit wrapping operations behave identically on all
        // three targets, including the full signed-i64 range.
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    pub fn integer(&mut self, low: i64, high: i64) -> Option<i64> {
        if low > high {
            return None;
        }
        let span = (high as i128 - low as i128 + 1) as u128;
        let domain = 1u128 << 64;
        let limit = domain - domain % span;
        loop {
            let value = self.next_u64() as u128;
            if value < limit {
                return Some((low as i128 + (value % span) as i128) as i64);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_sequence_and_full_range() {
        let mut random = RandomState::seeded(0);
        assert_eq!(random.next_u64(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(random.next_u64(), 0x6e78_9e6a_a1b9_65f4);
        for _ in 0..1000 {
            assert!((-4..=7).contains(&random.integer(-4, 7).unwrap()));
        }
        assert!(random.integer(i64::MIN, i64::MAX).is_some());
        assert_eq!(random.integer(4, 4), Some(4));
        let before = random;
        assert_eq!(random.integer(4, 3), None);
        assert_eq!(random, before);
    }
}

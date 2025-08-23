use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Money(pub i64);

impl Money {
    pub const SCALE: i64 = 10_000; // 4 decimal places
    pub const TARGET_DECIMALS: u32 = 4;

    pub fn zero() -> Self {
        Self(0)
    }
    pub fn as_minor(&self) -> i64 {
        self.0
    }

    pub fn from_scaled_i128(value: i128, scale: u32) -> Option<Self> {
        if scale == Self::TARGET_DECIMALS {
            if value < i128::from(i64::MIN) || value > i128::from(i64::MAX) {
                return None;
            }
            return Some(Self(value as i64));
        }
        if scale < Self::TARGET_DECIMALS {
            let diff = (Self::TARGET_DECIMALS - scale) as u32;
            let factor = 10i128.pow(diff);
            let widened = value.checked_mul(factor)?;
            if widened < i128::from(i64::MIN) || widened > i128::from(i64::MAX) {
                return None;
            }
            return Some(Self(widened as i64));
        }
        // scale > TARGET_DECIMALS: need rounding
        let diff = (scale - Self::TARGET_DECIMALS) as u32;
        let factor = 10i128.pow(diff);
        let div = value / factor; // truncated toward zero
        let rem = value % factor;
        if rem == 0 {
            if div < i128::from(i64::MIN) || div > i128::from(i64::MAX) {
                return None;
            }
            return Some(Self(div as i64));
        }
        let half = factor / 2;
        let abs_rem = rem.abs();
        let mut adjusted = div;
        if abs_rem > half {
            adjusted += if value.is_negative() { -1 } else { 1 };
        } else if abs_rem == half {
            // tie -> bankers (round half to even)
            if div & 1 != 0 {
                // previous is odd -> increment toward magnitude
                adjusted += if value.is_negative() { -1 } else { 1 };
            }
        }
        if adjusted < i128::from(i64::MIN) || adjusted > i128::from(i64::MAX) {
            return None;
        }
        Some(Self(adjusted as i64))
    }

    pub fn from_decimal_str(s: &str) -> Option<Self> {
        let s = s.trim();

        if s.is_empty() {
            return None;
        }
        let neg = s.starts_with('-');
        let body = s.trim_start_matches('-');
        let mut parts = body.split('.');
        let int_part = parts.next()?;
        if int_part.is_empty() {
            return None;
        }
        let int_val: i128 = int_part.parse().ok()?;
        let frac_opt = parts.next();
        if parts.next().is_some() {
            return None;
        }
        let (raw, scale) = if let Some(frac) = frac_opt {
            if frac.is_empty() {
                (int_val, 0)
            } else {
                (
                    int_val * 10i128.pow(frac.len() as u32) + frac.parse::<i128>().ok()?,
                    frac.len() as u32,
                )
            }
        } else {
            (int_val, 0)
        };
        let signed = if neg { -raw } else { raw };
        Money::from_scaled_i128(signed, scale)
    }
}

impl core::fmt::Display for Money {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let minor = self.0;
        let neg = minor < 0;
        let abs = minor.abs();
        let int_part = abs / Self::SCALE;
        let frac_part = abs % Self::SCALE;
        if neg {
            write!(f, "-{}.{:04}", int_part, frac_part)
        } else {
            write!(f, "{}.{:04}", int_part, frac_part)
        }
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Money::from_decimal_str(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("Invalid Money format: {}", s)))
    }
}

#[cfg(test)]
mod tests {
    use super::Money;
    #[test]
    fn bankers_round_half_even() {
        let v = Money::from_scaled_i128(1_23445, 5).unwrap(); // 1.23445 -> 1.2344
        assert_eq!(format!("{}", v), "1.2344");
        let v = Money::from_scaled_i128(1_23455, 5).unwrap(); // 1.23455 -> 1.2346
        assert_eq!(format!("{}", v), "1.2346");
        let v = Money::from_scaled_i128(-1_23445, 5).unwrap();
        assert_eq!(format!("{}", v), "-1.2344");
        let v = Money::from_scaled_i128(-1_23455, 5).unwrap();
        assert_eq!(format!("{}", v), "-1.2346");
    }
}

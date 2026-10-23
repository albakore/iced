/// An antialiasing strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Antialiasing {
    /// Multisample AA with 2 samples
    MSAAx2,
    /// Multisample AA with 4 samples
    MSAAx4,
    /// Multisample AA with 8 samples
    MSAAx8,
    /// Multisample AA with 16 samples
    MSAAx16,
}

impl Antialiasing {
    /// Returns the amount of samples of the [`Antialiasing`].
    pub fn sample_count(self) -> u32 {
        match self {
            Antialiasing::MSAAx2 => 2,
            Antialiasing::MSAAx4 => 4,
            Antialiasing::MSAAx8 => 8,
            Antialiasing::MSAAx16 => 16,
        }
    }
}

impl Antialiasing {
    /// pyzed: the [`Antialiasing`] with the given samples: 2, 4, 8 or 16 (other counts round
    /// to the nearest of them), or `None` for fewer than 2.
    pub fn from_samples(samples: u8) -> Option<Self> {
        Some(match samples {
            0 | 1 => return None,
            2 => Antialiasing::MSAAx2,
            3..=5 => Antialiasing::MSAAx4,
            6..=11 => Antialiasing::MSAAx8,
            _ => Antialiasing::MSAAx16,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Antialiasing;

    #[test]
    fn samples_round_to_a_supported_count() {
        assert_eq!(Antialiasing::from_samples(0), None);
        assert_eq!(Antialiasing::from_samples(2), Some(Antialiasing::MSAAx2));
        assert_eq!(Antialiasing::from_samples(4), Some(Antialiasing::MSAAx4));
        assert_eq!(Antialiasing::from_samples(8), Some(Antialiasing::MSAAx8));
        assert_eq!(Antialiasing::from_samples(32), Some(Antialiasing::MSAAx16));
    }
}

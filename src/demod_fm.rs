use num_complex::Complex32;
use std::f32::consts::TAU;

pub struct FmDemod {
    gain: f32,
    prev: Complex32,
}

impl FmDemod {
    pub fn new(deviation: u32, sample_rate: u32) -> FmDemod {
        assert!(deviation <= sample_rate / 2);

        FmDemod {
            gain: (TAU * deviation as f32 / sample_rate as f32).recip(),
            prev: Complex32::new(0.0, 0.0),
        }
    }

    #[inline]
    pub fn update(&mut self, sample: Complex32) -> f32 {
        let next = (sample * self.prev.conj()).arg() * self.gain;
        self.prev = sample;

        next
    }
}

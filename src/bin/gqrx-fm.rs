use anyhow::Result;
use anyhow::anyhow;
use num_complex::Complex32;
use shinelink::read_complex_f32;
use std::f32::consts::TAU;
use std::io::Write;
use std::{env, fs, io};

fn main() -> Result<()> {
    let usage = "usage: input_file samplerate deviation";
    let mut inp = io::BufReader::new(fs::File::open(
        env::args_os().nth(1).ok_or(anyhow!(usage))?,
    )?);

    let sample_rate = env::args().nth(2).ok_or(anyhow!(usage))?.parse::<u32>()?;
    let deviation = env::args().nth(3).ok_or(anyhow!(usage))?.parse::<u32>()?;
    let shift = env::args().nth(4).ok_or(anyhow!(usage))?.parse::<u32>()?;

    let mut out = io::BufWriter::new(fs::File::create("out.f32")?);

    let mut demod = FmDemod::new(deviation, sample_rate);

    // there's probably a dsp way to do this
    let mut i = 0f32;
    while let Some(mut sample) = read_complex_f32(&mut inp)? {
        i += 1.;
        sample *= Complex32::new(
            (TAU * shift as f32 / sample_rate as f32 * i).cos(),
            (TAU * shift as f32 / sample_rate as f32 * i).sin(),
        );
        let output = demod.update(sample);
        out.write_all(&output.to_le_bytes())?
    }

    Ok(())
}

struct FmDemod {
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

    pub fn update(&mut self, sample: Complex32) -> f32 {
        let next = (sample * self.prev.conj()).arg() * self.gain;
        self.prev = sample;

        next
    }
}

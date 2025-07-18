use anyhow::{Result, anyhow, ensure};
use itertools::Itertools;
use num_complex::Complex32;
use shinelink::demod_fm::FmDemod;
use shinelink::read_to_complex_f32;
use std::f32::consts::TAU;
use std::io::Write;
use std::{env, fs, io};

fn main() -> Result<()> {
    let decimation = 32;

    let usage = "usage: input_file samplerate deviation shift";
    let mut inp = io::BufReader::new(fs::File::open(
        env::args_os().nth(1).ok_or(anyhow!(usage))?,
    )?);

    let sample_rate = env::args().nth(2).ok_or(anyhow!(usage))?.parse::<u32>()?;
    let deviation = env::args().nth(3).ok_or(anyhow!(usage))?.parse::<u32>()?;
    let shift = env::args().nth(4).ok_or(anyhow!(usage))?.parse::<f64>()?;
    ensure!(
        deviation <= sample_rate / 2,
        "deviation must be less than half the sample rate"
    );
    ensure!(
        shift.abs() <= sample_rate as f64 / 2.,
        "shift must be less than half the sample rate"
    );

    let mut buf = Vec::with_capacity(64);
    let mut observations = Vec::new();
    let mut demod = FmDemod::new(deviation, sample_rate);

    let shift_rate = f64::from(TAU) * shift / sample_rate as f64;
    let mut i = 0f64;

    while let Some(mut sample) = read_to_complex_f32(&mut inp)? {
        i += 1.;
        sample *= Complex32::new((shift_rate * i).cos() as f32, (shift_rate * i).sin() as f32);
        buf.push(demod.update(sample));

        if buf.len() == decimation {
            // buf.sort_unstable_by(|a, b| f32::total_cmp(a, b));
            // let median = buf[buf.len() / 2];
            let mean = buf.iter().sum::<f32>() / buf.len() as f32;
            observations.push(mean);
            buf.truncate(0);
        }
    }

    let chunk_by = 16;

    let perfects = observations.chunks(chunk_by).map(perfect).collect_vec();
    let mut smoothed = perfects.clone();
    for i in 2..smoothed.len() - 2 {
        smoothed[i] |= perfects[i - 2..=i + 2].iter().any(|&v| v);
    }

    let mut nth = 0;
    let mut buf = Vec::new();
    for (v, chunk) in smoothed.into_iter().zip(observations.chunks(chunk_by)) {
        if v {
            buf.push(chunk);
            continue;
        }
        if buf.is_empty() {
            continue;
        }
        let mut file = fs::File::create(format!(
            "{nth}.squelch.sr{}.f32",
            sample_rate as usize / decimation
        ))?;
        for chunk in &buf {
            for obs in *chunk {
                file.write_all(&obs.to_le_bytes())?;
            }
        }

        nth += 1;
        buf.clear();
    }

    Ok(())
}

fn perfect(chunk: &[f32]) -> bool {
    let (min, max) = chunk
        .iter()
        .cloned()
        .minmax_by(f32::total_cmp)
        .into_option()
        .expect("non-empty chunk");

    max - min < 2.
}

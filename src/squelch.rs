use crate::demod_fm::FmDemod;
use crate::read_one_complex_f32;
use anyhow::{Result, ensure};
use itertools::Itertools;
use num_complex::Complex32;
use std::f32::consts::TAU;
use std::io::Read;

pub struct Config {
    /// how much to lowpass the signal
    pub decimation: usize,
    /// original sample rate, for timing / frequency purposes
    pub sample_rate: u32,
    /// how wide the signal is (Hz)
    pub deviation: u32,
    /// where the signal is, in the file (Hz)
    pub shift: f64,
}

/// reads cu8 samples, and extracts normalised, demodulated, decimated observations
pub fn squelch(inp: &mut impl Read, config: &Config) -> Result<Vec<(usize, Vec<f32>)>> {
    ensure!(
        config.deviation <= config.sample_rate / 2,
        "deviation must be less than half the sample rate"
    );
    ensure!(
        config.shift.abs() <= config.sample_rate as f64 / 2.,
        "shift must be less than half the sample rate"
    );

    let observations = read_shift_demod_decimate(inp, config)?;

    let chunk_by = 16;

    let perfects = observations.chunks(chunk_by).map(is_perfect).collect_vec();
    let smoothed = smooth(&perfects);

    let merged = merge_runs(&observations, &smoothed, chunk_by);
    Ok(merged)
}

fn merge_runs(observations: &[f32], smoothed: &[bool], chunk_by: usize) -> Vec<(usize, Vec<f32>)> {
    let mut picked = Vec::with_capacity(8);
    let mut buf = Vec::with_capacity(64);

    for (chunk_no, (v, chunk)) in smoothed
        .iter()
        .zip(observations.chunks(chunk_by))
        .enumerate()
    {
        if *v {
            buf.push(chunk);
            continue;
        }
        if buf.is_empty() {
            continue;
        }

        let concatenated = buf.iter().flat_map(|c| c.iter()).cloned().collect_vec();
        picked.push((chunk_no, normalise(&concatenated)));
        buf.clear();
    }

    picked
}

fn read_shift_demod_decimate(inp: &mut impl Read, config: &Config) -> anyhow::Result<Vec<f32>> {
    let mut demod = FmDemod::new(config.deviation, config.sample_rate);

    let mut buf = Vec::with_capacity(64);
    let mut observations = Vec::new();

    let shift_rate = f64::from(TAU) * config.shift / config.sample_rate as f64;
    let mut i = 0f64;

    while let Some(mut sample) = read_one_complex_f32(inp)? {
        i += 1.;
        sample *= Complex32::new((shift_rate * i).cos() as f32, (shift_rate * i).sin() as f32);
        buf.push(demod.update(sample));

        if buf.len() == config.decimation {
            // buf.sort_unstable_by(|a, b| f32::total_cmp(a, b));
            // let median = buf[buf.len() / 2];
            let mean = buf.iter().sum::<f32>() / buf.len() as f32;
            observations.push(mean);
            buf.truncate(0);
        }
    }
    Ok(observations)
}

fn smooth(orig: &[bool]) -> Vec<bool> {
    let mut smoothed = orig.to_vec();
    for i in 2..smoothed.len() - 2 {
        smoothed[i] |= orig[i - 2..=i + 2].iter().any(|&v| v);
    }
    smoothed
}

fn normalise(orig: &[f32]) -> Vec<f32> {
    let mut sorted = orig.to_vec();
    sorted.sort_unstable_by(f32::total_cmp);
    let percentile = 5;
    let low = sorted[sorted.len() * percentile / 100];
    let high = sorted[sorted.len() * (100 - percentile) / 100];

    let mid = (high + low) / 2.;
    let range = (high - low) / 2.;

    orig.iter().map(|v| (v - mid) / range).collect::<Vec<f32>>()
}

#[test]
fn test_normalise() {
    assert_eq_slice(&normalise(&[0., 0.5, 0., -0.5, 0.]), &[0., 1., 0., -1., 0.]);
    assert_eq_slice(
        &normalise(&[0.1, 0.6, 0.1, -0.4, 0.1]),
        &[0., 1., 0., -1., 0.],
    );
}

#[test]
fn test_normalise_sin() {
    fn sini(i: i32) -> f32 {
        (i as f32 * std::f32::consts::PI / 180.).sin()
    }

    let expected_scaling = 0.98897815;
    let expected_offset = 0.0012898743;

    let mut orig = (0..360).map(sini).collect::<Vec<f32>>();
    orig[17] = 25.;
    orig[170] = 0.;

    let mut expected = (0..360)
        .map(|i| sini(i) / expected_scaling - expected_offset)
        .collect::<Vec<f32>>();
    expected[17] = 25.277313;
    expected[170] = -0.0012898743;

    assert_eq_slice(&normalise(&orig), &expected);
}

#[cfg(test)]
fn assert_eq_slice(a: &[f32], b: &[f32]) {
    assert_eq!(a.len(), b.len());
    if let Some((i, (av, bv))) = a
        .iter()
        .zip(b.iter())
        .enumerate()
        .find(|(_, (a, b))| (*a - *b).abs() > 0.0001)
    {
        assert!(false, "not equal: {a:?} != {b:?}, at {i}, {av} != {bv}");
    }
}

fn is_perfect(chunk: &[f32]) -> bool {
    let (min, max) = chunk
        .iter()
        .cloned()
        .minmax_by(f32::total_cmp)
        .into_option()
        .expect("non-empty chunk");

    max - min < 2.
}

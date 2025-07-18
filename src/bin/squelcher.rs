use anyhow::{Result, anyhow, ensure};
use itertools::Itertools;
use num_complex::Complex32;
use shinelink::demod_fm::FmDemod;
use shinelink::read_to_complex_f32;
use std::f32::consts::TAU;
use std::io::Write;
use std::path::PathBuf;
use std::{env, fs, io};

fn main() -> Result<()> {
    let decimation = 32;

    let usage = "usage: input_file samplerate deviation shift";
    let original_name = PathBuf::from(env::args_os().nth(1).ok_or(anyhow!(usage))?);
    let original_file_name = original_name
        .file_name()
        .ok_or(anyhow!("input file must have a name"))?
        .to_string_lossy()
        .to_string();
    let mut inp = io::BufReader::new(fs::File::open(original_name)?);

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

        let flattened = normalise(&buf.iter().flat_map(|c| c.iter()).cloned().collect_vec());

        let decimated_sample_rate = sample_rate as usize / decimation;
        let name = format!(
            "{original_file_name}.{nth}.squelch.sr{}.f32",
            decimated_sample_rate
        );
        let mut file = io::BufWriter::new(fs::File::create(&name)?);

        for obs in &flattened {
            file.write_all(&obs.to_le_bytes())?;
        }
        file.flush()?;
        println!(
            "wrote {} samples to {}",
            flattened.len(),
            fs::canonicalize(name)?.display()
        );

        nth += 1;
        buf.clear();
    }

    Ok(())
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

fn perfect(chunk: &[f32]) -> bool {
    let (min, max) = chunk
        .iter()
        .cloned()
        .minmax_by(f32::total_cmp)
        .into_option()
        .expect("non-empty chunk");

    max - min < 2.
}

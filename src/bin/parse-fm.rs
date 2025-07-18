use anyhow::anyhow;
use anyhow::{Result, bail, ensure};
use itertools::Itertools;
use num_complex::Complex32;
use shinelink::demod_fm::FmDemod;
use shinelink::read_to_complex_f32;
use std::f32::consts::{PI, TAU};
use std::io::Write;
use std::{env, fs, io};

fn main() -> Result<()> {
    let decimation = 32;

    let usage = "usage: input_file samplerate deviation";
    let mut inp = io::BufReader::new(fs::File::open(
        env::args_os().nth(1).ok_or(anyhow!(usage))?,
    )?);

    let sample_rate = env::args().nth(2).ok_or(anyhow!(usage))?.parse::<u32>()?;
    let deviation = env::args().nth(3).ok_or(anyhow!(usage))?.parse::<u32>()?;
    let shift = env::args().nth(4).ok_or(anyhow!(usage))?.parse::<f32>()?;
    ensure!(
        deviation <= sample_rate / 2,
        "deviation must be less than half the sample rate"
    );
    ensure!(
        shift.abs() <= sample_rate as f32 / 2.,
        "shift must be less than half the sample rate"
    );

    let shift_rate = TAU * shift / sample_rate as f32;

    let mut out = io::BufWriter::new(fs::File::create(format!(
        "out.sr{}.f32",
        sample_rate as usize / decimation
    ))?);

    let mut demod = FmDemod::new(deviation, sample_rate);

    let mut buf = Vec::with_capacity(64);
    let mut observations = Vec::new();

    // there's probably a dsp way to do this
    let mut i = 0f32;
    while let Some(mut sample) = read_to_complex_f32(&mut inp)? {
        i += 1.;
        sample *= Complex32::new((shift_rate * i).cos(), (shift_rate * i).sin());
        buf.push(demod.update(sample));

        if buf.len() >= decimation {
            let mean = buf.iter().sum::<f32>() / buf.len() as f32;
            out.write_all(&mean.to_le_bytes())?;
            observations.push(mean);
            buf.truncate(0);
        }
    }

    let in_range_values = 32;
    let start = match observations.chunks_exact(in_range_values).position(|w| {
        let (min, max) = w
            .iter()
            .copied()
            .minmax()
            .into_option()
            .expect("non-empty chunk");
        max - min < 2.
    }) {
        Some(pos) => pos * in_range_values,
        None => bail!("no stability found in input file"),
    };

    let observations = &observations[start..];

    let mut cut = io::BufWriter::new(fs::File::create(format!(
        "cut.sr{}.f32",
        sample_rate as usize / decimation
    ))?);

    for v in observations {
        cut.write_all(&v.to_le_bytes())?;
    }

    let mut diff = io::BufWriter::new(fs::File::create(format!(
        "diff.sr{}.f32",
        sample_rate as usize / decimation
    ))?);

    for v in observations.iter().tuple_windows().map(|(f, s)| s - f) {
        diff.write_all(&v.to_le_bytes())?;
    }

    // sign_flip(observations);
    let slide_length = sample_rate as f32 / decimation as f32 / 10_000.;
    println!("slide length: {slide_length}");

    // TODO: magic
    let orig_amp = 0.65;

    let slide_pos = (0..=slide_length.round() as usize)
        .map(|i| orig_amp * ((PI * ((i as f32) / slide_length - 0.5)).sin()))
        .collect::<Vec<f32>>();
    let slide_neg = slide_pos.iter().rev().cloned().collect_vec();

    let mut mix = io::BufWriter::new(fs::File::create(format!(
        "mix.sr{}.f32",
        sample_rate as usize / decimation
    ))?);

    let mut mixed = Vec::new();

    for v in observations.windows(slide_pos.len()) {
        let mut pos = 0f32;
        let mut neg = 0f32;
        for (i, v) in v.iter().enumerate() {
            pos += (v - slide_pos[i]).abs();
            neg += (v - slide_neg[i]).abs();
        }
        pos /= slide_pos.len() as f32;
        neg /= slide_neg.len() as f32;
        let both = neg - pos;
        mixed.push(both);
        mix.write_all(&both.to_le_bytes())?;
    }

    let mut status_file = io::BufWriter::new(fs::File::create(format!(
        "status.sr{}.f32",
        sample_rate as usize / decimation
    ))?);

    let mut out_bits = Vec::new();

    let mut prev = 0;
    let mut buf = Vec::new();
    for (i, &v) in mixed.iter().enumerate() {
        let mut status = 0f32;
        if v.abs() > 0.5 {
            buf.push((i, v));
        } else {
            if let Some((pos, v)) = buf
                .iter()
                .max_by(|(_, a), (_, b)| f32::total_cmp(&a.abs(), &b.abs()))
            {
                let bits = (pos - prev) as f32 / 10.5;
                let error = bits - bits.round();
                let error_perc = error.abs() * 100.;
                status = error;
                println!(
                    "{:6.3} at {:3} ({:6.3}), {:10} from {}",
                    v,
                    pos - prev,
                    bits,
                    if error_perc > 20. {
                        format!("error: {error_perc:2.0}")
                    } else {
                        String::new()
                    },
                    buf.len()
                );
                for _ in 0..bits.round() as usize {
                    out_bits.push(v.is_sign_negative());
                }
                prev = *pos;
            }
            buf.truncate(0);
        }

        status_file.write_all(&status.to_le_bytes())?;
    }

    for offset in 0..8 {
        println!(
            "{offset}: {:?}",
            out_bits[offset..]
                .chunks_exact(8)
                .map(bits_to_byte)
                .map(char::from)
                .collect::<String>()
        );
    }

    Ok(())
}

fn bits_to_byte(bits: &[bool]) -> u8 {
    bits.iter()
        .rev()
        .enumerate()
        .map(|(i, &b)| if b { 1 << i } else { 0 })
        .sum()
}

#[allow(dead_code)]
fn sign_flip(observations: &[f32]) {
    let mut current_sign = false;
    let mut run = 0;

    for &v in observations {
        if v.abs() < 0.3 {
            continue;
        }
        // println!("{:6.3}", v);
        if v.is_sign_positive() == current_sign {
            run += 1;
        } else {
            println!("{}: {}", if current_sign { "  up" } else { "down" }, run);
            current_sign = !current_sign;
            run = 1;
        }
    }
}

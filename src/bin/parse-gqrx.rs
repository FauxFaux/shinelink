use anyhow::{Result, anyhow};

use itertools::Itertools;
use num_complex::Complex;
use rustfft::FftPlanner;
use shinelink::read_complex_f32;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, Read};
use std::{env, fs, io};

fn main() -> Result<()> {
    let mut inp = io::BufReader::new(fs::File::open(
        env::args_os().nth(1).ok_or(anyhow!("usage: input file"))?,
    )?);
    let fft = FftPlanner::new().plan_fft_forward(256);
    let mut buf = VecDeque::new();
    let window = generate_blackman_harris_window(fft.len());

    let mut recent_maxes = VecDeque::new();
    let mut centre: Option<i16> = None;
    let mut captured = Vec::new();

    let mut i = 0;
    'app: loop {
        i += 1;
        while buf.len() < fft.len() {
            match read_complex_f32(&mut inp)? {
                Some(c) => buf.push_back(c),
                None => {
                    break 'app; // end of file
                }
            }
        }

        let mut sub = buf.iter().cloned().collect_vec();

        for (c, w) in sub.iter_mut().zip(window.iter()) {
            *c *= w;
        }

        fft.process(&mut sub);

        let (max_pos, _) = sub
            .iter()
            .skip(1)
            .map(|v| v.norm())
            .enumerate()
            .max_by(|(_, a), (_, b)| f32::total_cmp(&a, &b))
            .expect("sub.len() > 0");

        recent_maxes.push_back(max_pos as i16);
        while recent_maxes.len() > 16 {
            recent_maxes.pop_front();
        }

        match centre {
            Some(already_capturing) => {
                // probably better doing 90th percentile?
                let sum: i64 = recent_maxes
                    .iter()
                    .map(|m| i64::from(already_capturing - *m).abs())
                    .sum();
                if sum < 16 * 10 {
                    let cnt = already_capturing as usize;
                    // this doesn't work well; the power of the blur/gfsk overwhelms the power of the signal
                    // let low_power = total_power(&sub[cnt - 5..cnt - 2]);
                    // let high_power = total_power(&sub[cnt + 2..cnt + 5]);
                    // captured.push(high_power - low_power);
                    captured.push(max_pos as f32 - cnt as f32);
                } else {
                    recover(&captured)?;
                    centre = None;
                    captured.truncate(0);
                }
            }
            None => {
                let (min, max) = recent_maxes
                    .iter()
                    .cloned()
                    .minmax()
                    .into_option()
                    .expect("just pushed");
                let range = max - min;
                if range > 5 && range < 10 {
                    println!("Found centre at {min}..{max} (range {range})");
                    centre = Some(min + range / 2);
                }
            }
        }

        for _ in 0..16 {
            let _ = buf.pop_front();
        }
    }

    Ok(())
}

fn recover(vals: &[f32]) -> Result<()> {
    let mut runs = Vec::new();
    let mut current_sign = false;
    let mut current_run = 0usize;
    for v in vals {
        if v.abs() < 2. {
            continue;
        }
        let this_sign = v.is_sign_positive();
        if this_sign == current_sign {
            current_run += 1;
        } else {
            if current_run > 0 {
                runs.push((current_sign, current_run));
            }
            current_sign = this_sign;
            current_run = 1;
        }
    }
    println!("Runs: {runs:?}");

    let premable = &runs[2..20];
    let trues = length_guess(premable, true);
    let falses = length_guess(premable, false);

    for (sign, len) in runs {
        println!(
            "{}: {:.2}",
            if sign { "1" } else { "0" },
            len as f32 / if sign { trues } else { falses }
        );
    }

    Ok(())
}

fn length_guess(premable: &[(bool, usize)], target: bool) -> f32 {
    let examples = premable
        .iter()
        .filter_map(|(sign, v)| if *sign == target { Some(*v) } else { None })
        .collect_vec();
    examples.iter().sum::<usize>() as f32 / examples.len() as f32
}

pub fn generate_blackman_harris_window(n: usize) -> Vec<f32> {
    let mut window = Vec::with_capacity(n);
    for i in 0..n {
        let x = std::f32::consts::TAU * i as f32 / (n - 1) as f32;
        let value =
            0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos() - 0.01168 * (3.0 * x).cos();
        window.push(value);
    }
    window
}

fn total_power(buf: &[Complex<f32>]) -> f32 {
    buf.iter().map(|v| v.norm()).sum()
}

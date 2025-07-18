use anyhow::Result;
use itertools::Itertools;
use memchr::memmem;
use shinelink::bits_to_byte;
use std::collections::HashSet;
use std::f32::consts::PI;
use std::fs;

fn main() -> Result<()> {
    let usage = "usage: decode file samplerate";

    let input = fs::read(
        std::env::args()
            .nth(1)
            .ok_or_else(|| anyhow::anyhow!(usage))?,
    )?
    .chunks_exact(4)
    .map(|v| f32::from_le_bytes(<[u8; 4]>::try_from(v).expect("chunks_exact")))
    .collect_vec();

    let sample_rate = std::env::args()
        .nth(2)
        .ok_or_else(|| anyhow::anyhow!(usage))?
        .parse::<u32>()?;

    // 100us transitions
    let edge_length = sample_rate as f32 / (1_000_000. / 100.);
    println!("{edge_length}");

    let differential = detect_edges(&input, edge_length);
    let runs = find_runs(&differential);

    let mut candidate_bytes = HashSet::with_capacity(4);

    // e.g. 17.0, 17.1,.. to 19.0
    for clock in (-20..30).map(|v| edge_length + (v as f32) / 10.) {
        let mut bits = Vec::new();
        for (run_length, is_positive) in &runs {
            for _ in 0..(*run_length as f32 / clock).round() as usize {
                // TODO: suspicious bang
                bits.push(!*is_positive);
            }
        }
        for offset in 0..8 {
            let cand = bits[offset..]
                .chunks_exact(8)
                .map(bits_to_byte)
                .collect_vec();
            if let Some(jack) = memmem::find(&cand, b"jack") {
                println!("Found 'jack' at offset {offset} with clock {clock}");
                candidate_bytes.insert(cand[jack + "jack".len()..].to_vec());
            }
        }
    }

    let key = b"GROWATTRF.";

    let crc = crc::Crc::<u16>::new(&crc::CRC_16_MODBUS);

    let mut matches_crc = HashSet::new();

    for cand in &candidate_bytes {
        // println!("Decoded: {:?}", unambiguous(&cand));
        for offset in 0..key.len() {
            let decrypted = cand
                .iter()
                .zip(key.iter().cycle().skip(offset))
                .map(|(&c, &k)| c ^ k)
                .collect_vec();
            if likely_valid(&decrypted) {
                println!(
                    "{offset}: {} {}",
                    unambiguous(&decrypted),
                    hex::encode(&decrypted)
                );
                for i in (1..decrypted.len() - 2).rev() {
                    let crc_bytes = &decrypted[1..i];
                    let expected = u16::from_be_bytes([decrypted[i], decrypted[i + 1]]);
                    if crc.checksum(crc_bytes) == expected {
                        println!(
                            "CRC VALID!!!! at offset {offset}: {}",
                            unambiguous(crc_bytes)
                        );
                        matches_crc.insert(crc_bytes.to_vec());
                    }
                }
            }
        }
    }

    println!("{} valid", matches_crc.len());
    for cand in &matches_crc {
        println!("CRC matches: {} {}", unambiguous(cand), hex::encode(cand));
    }

    Ok(())
}

fn likely_valid(input: &[u8]) -> bool {
    input.windows(20).any(|w| {
        w.iter()
            .all(|v| v.is_ascii_digit() || v.is_ascii_uppercase())
    }) || input.windows(10).any(|w| w.iter().all(|&v| v == 0))
}

/// detect edges in a "time domain" signal, outputting how close we are to a positive or negative edge
/// e.g.
/// ```text
///          *******
///         *
///        *
///       *
/// ******
/// ```
///
/// ```text
/// input: -1, -1, -1, -1, -1, -1,  -0.5, 0, 0.5, 1, 1, 1, 1, 1, 1
/// output: 0,  0,  0,  0,  0,  0.2, 0.5, 1, 0.5, 0, 0, 0, 0, 0, 0
/// ```
///
/// i.e
/// ```text
///        *
///       * *
/// ******   ******
/// ```
///
/// ...give or take some offsets
fn detect_edges(input: &[f32], edge_length: f32) -> Vec<f32> {
    let edge_pos = (0..=edge_length.round() as usize)
        .map(|i| (PI * ((i as f32) / edge_length - 0.5)).sin())
        .collect_vec();
    let edge_neg = edge_pos.iter().rev().cloned().collect_vec();

    input
        .windows(edge_pos.len())
        .map(|v| {
            let mut pos = 0f32;
            let mut neg = 0f32;
            for (i, v) in v.iter().enumerate() {
                pos += (v - edge_pos[i]).abs();
                neg += (v - edge_neg[i]).abs();
            }
            pos /= edge_pos.len() as f32;
            neg /= edge_neg.len() as f32;

            neg - pos
        })
        .collect()
}

/// find the peaks in a differential signal, returning the position and whether the peak is positive or negative
///
/// e.g.
/// ```text
///              *                   *
///             * *                 * *
/// ************   *******   *******   *
///                       * *
///                        *
///    5    10    15    20    25    30    35
/// ```
///
/// ```text
/// output: (14, ?), ((24-14), true), ((30-24), false)
///
fn find_runs(differential: &[f32]) -> Vec<(usize, bool)> {
    let mut runs = Vec::new();
    let mut prev = 0;
    let mut buf = Vec::new();
    for (i, &v) in differential.iter().enumerate() {
        if v.abs() > 0.5 {
            buf.push((i, v));
        } else {
            if let Some((pos, v)) = buf
                .iter()
                .max_by(|(_, a), (_, b)| f32::total_cmp(&a.abs(), &b.abs()))
            {
                runs.push((pos - prev, v.is_sign_positive()));
                prev = *pos;
            }
            buf.truncate(0);
        }
    }
    runs
}

fn unambiguous(input: &[u8]) -> String {
    let mut buf = String::with_capacity(2 * input.len());
    for &c in input {
        if c.is_ascii_alphanumeric() || c.is_ascii_punctuation() {
            buf.push(char::from(c));
        } else {
            buf.push_str(&format!("[{c}]"));
        }
        buf.push(' ');
    }
    buf.pop();
    buf
}

#[test]
fn test_crc() {
    // the library has completely changed their api again, haven't they. hth hand
    let crc = crc::Crc::<u16>::new(&crc::CRC_16_MODBUS);
    assert_eq!(crc.checksum(b"123456789"), 19255);
    assert_eq!(crc.checksum(b"12345678"), 14301);
}

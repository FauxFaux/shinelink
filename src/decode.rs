use crate::bits_to_byte;
use crate::crc::crc_suffixed;
use itertools::Itertools;
use memchr::memmem;
use std::collections::HashSet;
use std::f32::consts::PI;

const KNOWN_HEADER_BYTES: &[u8; 4] = b"jack";
const ENCRYPTION_KEY: &[u8; 10] = b"GROWATTRF.";

pub fn decode(input: &[f32], edge_length: f32) -> (HashSet<Vec<u8>>, HashSet<Vec<u8>>) {
    let differential = detect_edges(input, edge_length);
    let runs = find_runs(&differential);

    let candidate_bytes = recover_bytes(&runs, edge_length);

    let (matches_crc, looks_plausible) = attempt_decrypt(&candidate_bytes);
    (matches_crc, looks_plausible)
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

/// given a bunch of pulse lengths, and a bit length, find some clocks and offsets of bits
/// which result in byte streams which contain the known header bytes
fn recover_bytes(runs: &[(usize, bool)], edge_length: f32) -> HashSet<Vec<u8>> {
    let mut candidate_bytes = HashSet::with_capacity(4);

    // e.g. 15.00, 15.01,.. to 21.00
    for clock in (-300..300).map(|v| edge_length + (v as f32) / 100.) {
        let mut bits = Vec::with_capacity(runs.len() * 6);
        for (run_length, is_positive) in runs {
            for _ in 0..(*run_length as f32 / clock).round() as usize {
                // TODO: suspicious bang
                bits.push(!*is_positive);
            }
        }

        if bits.len() < 32 {
            continue;
        }

        for offset in 0..8 {
            let cand = bits[offset..]
                .chunks_exact(8)
                .map(bits_to_byte)
                .collect_vec();

            let header = KNOWN_HEADER_BYTES;
            if let Some(jack) = memmem::find(&cand, header) {
                candidate_bytes.insert(cand[jack + header.len()..].to_vec());
            }
        }
    }

    candidate_bytes
}

/// classify the candidates by whether we can decrypt them to strings matching the crc,
/// and whether they look plausible after some decryption
///
/// note that, on bit alignment errors, the second half of the packet may decrypt with a different offset.
fn attempt_decrypt(candidate_bytes: &HashSet<Vec<u8>>) -> (HashSet<Vec<u8>>, HashSet<Vec<u8>>) {
    let key = ENCRYPTION_KEY;

    let mut matches_crc = HashSet::with_capacity(4);
    let mut looks_plausible = HashSet::with_capacity(16);
    for cand in candidate_bytes {
        for offset in 0..key.len() {
            let decrypted = cand
                .iter()
                .zip(key.iter().cycle().skip(offset))
                .map(|(&c, &k)| c ^ k)
                .collect_vec();

            for i in (1..decrypted.len()).rev() {
                if let Some(crc_bytes) = crc_suffixed(&decrypted[1..i]) {
                    matches_crc.insert(crc_bytes.to_vec());
                }
            }

            if likely_valid(&decrypted) {
                looks_plausible.insert(decrypted.clone());
            }
        }
    }

    (matches_crc, looks_plausible)
}

/// if it contains the serial pair, or a long run of nulls, it's probably at least interesting
fn likely_valid(input: &[u8]) -> bool {
    input.windows(20).any(|w| {
        w.iter()
            .all(|v| v.is_ascii_digit() || v.is_ascii_uppercase())
    }) || input.windows(10).any(|w| w.iter().all(|&v| v == 0))
}

pub mod crc;
pub mod decode;
pub mod demod_fm;
pub mod squelch;

use anyhow::Result;
use num_complex::Complex;
use std::io::Read;
use std::path::Path;
use std::{fs, io};

pub fn read_one_complex_f32(inp: &mut impl Read) -> Result<Option<Complex<f32>>> {
    let mut buf = [0u8; 2];
    if let Err(e) = inp.read_exact(&mut buf) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e.into());
    }
    Ok(Some(Complex::new(u8_to_f32(buf[0]), u8_to_f32(buf[1]))))
}

pub fn read_to_end_f32(input: impl AsRef<Path>) -> Result<Vec<f32>> {
    Ok(fs::read(input)?
        .chunks_exact(4)
        .map(|v| f32::from_le_bytes(<[u8; 4]>::try_from(v).expect("chunks_exact")))
        .collect())
}

fn u8_to_f32(v: u8) -> f32 {
    (v as f32 - 128.0) / 128.0
}

pub fn bits_to_byte(bits: &[bool]) -> u8 {
    assert_eq!(bits.len(), 8);
    bits.iter()
        .rev()
        .enumerate()
        .map(|(i, &b)| if b { 1 << i } else { 0 })
        .sum()
}

pub fn unambiguous(input: &[u8]) -> String {
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

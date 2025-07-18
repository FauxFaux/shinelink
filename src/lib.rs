pub mod crc;
pub mod demod_fm;
pub mod squelch;

use anyhow::Result;
use num_complex::Complex;
use std::io;
use std::io::Read;

pub fn read_to_complex_f32(inp: &mut impl Read) -> Result<Option<Complex<f32>>> {
    let mut buf = [0u8; 2];
    if let Err(e) = inp.read_exact(&mut buf) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e.into());
    }
    Ok(Some(Complex::new(u8_to_f32(buf[0]), u8_to_f32(buf[1]))))
}

fn u8_to_f32(v: u8) -> f32 {
    (v as f32 - 128.0) / 128.0
}

pub fn bits_to_byte(bits: &[bool]) -> u8 {
    bits.iter()
        .rev()
        .enumerate()
        .map(|(i, &b)| if b { 1 << i } else { 0 })
        .sum()
}

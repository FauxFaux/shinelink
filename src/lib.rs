pub mod demod_fm;

use anyhow::Result;
use num_complex::Complex;
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};

pub fn read_to_complex_f32(inp: &mut BufReader<File>) -> Result<Option<Complex<f32>>> {
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

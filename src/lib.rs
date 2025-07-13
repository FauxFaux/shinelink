use anyhow::Result;
use num_complex::Complex;
use std::fs::File;
use std::io;
use std::io::{BufReader, Read};

pub fn read_complex_f32(inp: &mut BufReader<File>) -> Result<Option<Complex<f32>>> {
    let mut re = [0u8; 4];
    let mut im = [0u8; 4];
    if let Err(e) = inp.read_exact(&mut re) {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            return Ok(None);
        }
        return Err(e.into());
    }
    inp.read_exact(&mut im)?;
    let complex = Complex::new(f32::from_le_bytes(re), f32::from_le_bytes(im));
    Ok(Some(complex))
}

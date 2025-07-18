use anyhow::{Context, Result};
use itertools::Itertools;
use shinelink::decode::decode;
use shinelink::squelch::{Config, squelch};
use shinelink::unambiguous;
use std::path::PathBuf;
use std::{fs, io};
use std::io::Write;

#[derive(facet::Facet)]
struct Args {
    #[facet(positional)]
    input_dir: PathBuf,
}

fn main() -> Result<()> {
    let args: Args = facet_args::from_std_args().context("usage: many-packets input_dir")?;
    for f in fs::read_dir(&args.input_dir)? {
        let f = f?;
        if !f.file_type()?.is_file() || !f.file_name().to_string_lossy().ends_with(".cu8") {
            continue;
        }
        let fms = squelch(
            &mut io::BufReader::new(fs::File::open(f.path())?),
            &Config {
                decimation: 16,
                sample_rate: 2_880_000,
                deviation: 60_000,
                shift: 476_000.,
            },
        )?;
        for (n, fm) in fms {
            let (crc, _rest) = decode(&fm, 18.);
            let good = crc.iter().filter(|v| v.starts_with(b"RF") && v.len() > 30).collect_vec();
            for good in good {
                let sn = &good[6..26];
                if !sn.iter().all(|v| v.is_ascii_uppercase() || v.is_ascii_digit()) {
                    continue;
                }

                let data = good[29..].to_vec();
                if data.len() < 4 {
                    continue;
                }
                let pkt = Pkt {
                    seq: good[2],
                    prefix: good[3..6].try_into()?,
                    sn: String::from_utf8_lossy(&sn).to_string(),
                    req: u16::from_be_bytes(good[27..=28].try_into()?),
                };
                println!(
                    "{} {n:6} {:?} {}",
                    f.file_name().display(),
                    pkt,
                    unambiguous(&data),
                );
                let mut file = fs::File::create(format!("{}.{n}.{}.pkt", f.file_name().display(), pkt.req))?;
                file.write_all(&data)?;
                file.flush()?;
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Pkt {
    seq: u8,
    prefix: [u8; 3],
    sn: String,
    req: u16,
}
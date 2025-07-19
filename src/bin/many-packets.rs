use anyhow::{Context, Result};
use itertools::Itertools;
use shinelink::bulk::bulk_process;
use shinelink::decode::decode;
use shinelink::squelch::Config;
use shinelink::unambiguous;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(facet::Facet)]
struct Args {
    #[facet(positional)]
    input_dir: PathBuf,
}

fn main() -> Result<()> {
    let args: Args = facet_args::from_std_args().context("usage: many-packets input_dir")?;

    let config = Config {
        decimation: 16,
        sample_rate: 2_880_000,
        deviation: 60_000,
        shift: 476_000.,
    };

    bulk_process(capture_very_high_quality_packets, &args.input_dir, &config)?;

    Ok(())
}

fn capture_very_high_quality_packets(file_name: &str, fms: &[(usize, Vec<f32>)]) -> Result<()> {
    for (n, fm) in fms {
        let (crc, _rest) = decode(&fm, 18.);
        let good = crc
            .iter()
            .filter(|v| v.starts_with(b"RF") && v.len() > 30)
            .collect_vec();

        for good in good {
            let sn = &good[6..26];
            if !sn
                .iter()
                .all(|v| v.is_ascii_uppercase() || v.is_ascii_digit())
            {
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
            println!("{} {n:6} {:?} {}", file_name, pkt, unambiguous(&data),);
            let mut file = fs::File::create(format!("{}.{n}.{}.pkt", file_name, pkt.req))?;
            file.write_all(&data)?;
            file.flush()?;
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

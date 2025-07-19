use anyhow::{Context, Result};
use shinelink::bulk::bulk_process;
use shinelink::decode::decode;
use shinelink::squelch::Config;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(facet::Facet)]
struct Args {
    #[facet(positional)]
    input_dir: PathBuf,
}

fn main() -> Result<()> {
    let args: Args = facet_args::from_std_args().context("usage: find-172 input_dir")?;

    let config = Config {
        decimation: 16,
        sample_rate: 2_880_000,
        deviation: 60_000,
        shift: 476_000.,
    };

    bulk_process(find_172, &args.input_dir, &config)?;

    Ok(())
}

fn find_172(file_name: &str, fms: &[(usize, Vec<f32>)]) -> Result<()> {
    for (offset, fm) in fms {
        // all perfect examples we've seen are between 32448 and 32512, so this is quite a wide window
        if fm.len() < 30_000 || fm.len() > 34_000 {
            continue;
        }
        let (crc, rest) = decode(&fm, 18.);
        println!(
            "{file_name:65} {offset:6} {:6} {:?}",
            fm.len(),
            classify(crc, rest)?
        );
    }
    Ok(())
}

#[derive(Debug)]
enum Outcome {
    Perfect(u8),
    Plausible(u8),
    Bad(u8),
    None,
}

fn classify(crc: HashSet<Vec<u8>>, rest: HashSet<Vec<u8>>) -> Result<Option<Outcome>> {
    let mut crc_matches = HashSet::new();
    for sample in &crc {
        if !sn_packet(&sample) {
            continue;
        }
        crc_matches.insert(sample[28]);
    }
    if crc_matches.len() == 1 {
        let seq = crc_matches.iter().next().unwrap();
        return Ok(Some(Outcome::Perfect(*seq)));
    }
    Ok(None)
}

fn sn_packet(sample: &[u8]) -> bool {
    if !sample.starts_with(b"RF") || sample.len() < 30 {
        return false;
    }
    let sn = &sample[6..26];
    if !sn
        .iter()
        .all(|v| v.is_ascii_uppercase() || v.is_ascii_digit())
    {
        return false;
    }

    true
}

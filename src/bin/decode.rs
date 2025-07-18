use anyhow::{Context, Result};
use shinelink::decode::decode;
use shinelink::{read_to_end_f32, unambiguous};
use std::path::PathBuf;

#[derive(facet::Facet)]
struct Args {
    #[facet(positional)]
    file: PathBuf,

    #[facet(positional)]
    sample_rate: u32,
}

fn main() -> Result<()> {
    let usage = "usage: decode file samplerate";

    let args: Args = facet_args::from_std_args().context(usage)?;

    let input = read_to_end_f32(&args.file)?;

    // 100us transitions
    let edge_length = args.sample_rate as f32 / (1_000_000. / 100.);

    let (matches_crc, looks_plausible) = decode(&input, edge_length);

    if !matches_crc.is_empty() {
        for cand in &matches_crc {
            println!("match: {} // {}", unambiguous(cand), hex::encode(cand));
        }
    } else {
        for decrypted in &looks_plausible {
            println!(
                "no match: {} // {}",
                unambiguous(decrypted),
                hex::encode(decrypted)
            );
        }
    }

    Ok(())
}

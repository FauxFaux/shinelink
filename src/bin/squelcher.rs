use anyhow::{Context, Result, anyhow};
use shinelink::squelch::{Config, squelch};
use std::io::Write;
use std::path::PathBuf;
use std::{fs, io};

#[derive(facet::Facet)]
struct Args {
    #[facet(positional)]
    path: PathBuf,

    #[facet(positional)]
    sample_rate: u32,

    #[facet(positional)]
    deviation: u32,

    #[facet(positional)]
    shift: f64,
}

fn main() -> Result<()> {
    let args: Args =
        facet_args::from_std_args().context("usage: file sample_rate deviation shift")?;

    let original_file_name = args
        .path
        .file_name()
        .ok_or(anyhow!("input file must have a name"))?
        .to_string_lossy()
        .to_string();
    let mut inp = io::BufReader::new(fs::File::open(args.path)?);

    let config = Config {
        decimation: 16,
        sample_rate: args.sample_rate,
        deviation: args.deviation,
        shift: args.shift,
    };

    let merged = squelch(&mut inp, &config)?;

    let decimated_sample_rate = config.sample_rate as usize / config.decimation;

    for (nth, flattened) in merged.into_iter() {
        let name = format!("{original_file_name}.{nth}.squelch.sr{decimated_sample_rate}.f32");
        let mut file = io::BufWriter::new(fs::File::create(&name)?);

        for obs in &flattened {
            file.write_all(&obs.to_le_bytes())?;
        }
        file.flush()?;

        println!(
            "wrote {} samples to {}",
            flattened.len(),
            fs::canonicalize(name)?.display()
        );
    }

    Ok(())
}

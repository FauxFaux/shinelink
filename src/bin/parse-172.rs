use anyhow::{Context, Result};
use itertools::Itertools;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

#[derive(facet::Facet)]
struct Args {
    #[facet(positional)]
    input_dir: PathBuf,
}

fn main() -> Result<()> {
    let args: Args = facet_args::from_std_args().context("usage: many-packets input_dir")?;
    let mut examples = Vec::new();
    for f in fs::read_dir(&args.input_dir)? {
        let f = f?;
        if !f.file_type()?.is_file() || !f.file_name().to_string_lossy().ends_with(".pkt") {
            continue;
        }
        let bytes = fs::read(f.path())?;
        examples.push((
            f.file_name().display().to_string(),
            bytes
                .chunks_exact(4)
                .map(|v| i32::from_be_bytes(v.try_into().expect("chunks_exact")))
                .collect_vec(),
        ));
    }

    let longest = examples.iter().map(|(_, v)| v.len()).max().unwrap_or(0);
    let mut values = (0..longest).map(|_| HashSet::<i32>::new()).collect_vec();
    for (_, data) in &examples {
        for (i, &value) in data.iter().enumerate() {
            values[i].insert(value);
        }
    }

    for (i, set) in values.iter().enumerate() {
        println!(
            "Column {i}: {} unique values: {:?}",
            set.len(),
            set.iter().sorted().collect_vec()
        );
    }

    println!();

    let interesting = values
        .iter()
        .enumerate()
        .filter(|(i, value)| value.len() > 1)
        .map(|(i, _)| i)
        .collect_vec();

    examples.sort_unstable();

    for (name, data) in examples {
        let mut buf = String::new();
        for &i in &interesting {
            buf.push_str(&format!("{i:2}: {:6}  ", data[i]));
        }

        println!("{name:20}:  {buf}");
    }

    Ok(())
}

use crate::squelch::{Config, squelch};
use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;
use std::{fs, io};

pub fn bulk_process<T: Send>(
    func: impl Sync + Send + Fn(&str, &[(usize, Vec<f32>)]) -> Result<T>,
    input_dir: impl AsRef<Path>,
    config: &Config,
) -> Result<Vec<T>> {
    let mut files = Vec::new();
    for f in fs::read_dir(input_dir)? {
        let f = f?;
        if !f.file_type()?.is_file() {
            continue;
        }
        let path = f.path();
        if path.extension() != Some("cu8".as_ref()) {
            continue;
        }
        files.push(f.path());
    }

    files
        .into_par_iter()
        .map(|f| -> anyhow::Result<T> {
            let file_name = f
                .file_name()
                .expect("dir entries have names")
                .display()
                .to_string();

            let fms = squelch(&mut io::BufReader::new(fs::File::open(&f)?), &config)?;
            func(&file_name, &fms)
        })
        .collect()
}

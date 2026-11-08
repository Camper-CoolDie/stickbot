use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write as _};
use std::path::PathBuf;

use anyhow::Result;

fn main() -> Result<()> {
    let in_path = PathBuf::from("src/strings.toml");
    let out_path = PathBuf::from(env::var("OUT_DIR")?).join("strings.rs");
    println!("cargo::rerun-if-changed={}", in_path.display());

    let data = fs::read(in_path)?;
    let strings = toml::from_slice::<HashMap<String, String>>(&data)?;

    let file = File::create(out_path)?;
    let mut writer = BufWriter::new(file);
    for (key, string) in strings {
        writeln!(
            writer,
            "pub const {}: &str = {string:?};",
            key.to_uppercase()
        )?;
    }

    writer.flush()?;
    Ok(())
}

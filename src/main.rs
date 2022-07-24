#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]

mod csv;
mod utils;

use std::{env, fs::File, io::BufReader};

use anyhow::{anyhow, Result};

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("Missing path to csv"))?;

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    csv::process(reader)
}

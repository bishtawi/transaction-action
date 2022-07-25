#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]

mod dtos;
mod processor;
mod stores;

use std::{env, fs::File, io::BufReader};

use processor::Processor;

use anyhow::{anyhow, Result};

fn main() -> Result<()> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| anyhow!("Missing path to csv"))?;

    let file = File::open(path)?;
    let reader = BufReader::new(file);

    // Set up processing engine
    // If our processing engine was connecting to a real database, setting up database connections would happen here
    // Instead we can use the `default` implementation which will set us up with in-memory datastores
    let mut processor = Processor::default();

    processor.handle(reader);

    // TODO: Print final csv
    todo!()
}

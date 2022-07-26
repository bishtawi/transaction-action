#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]

use std::{
    env,
    fs::File,
    io::{self, BufReader},
};
use transaction_action::CSVProcessor;

fn main() {
    let path = env::args()
        .nth(1)
        .expect("First argument should be path to csv file");

    let file = File::open(path).expect("Unable to open file");
    let reader = BufReader::new(file);

    let mut processor = CSVProcessor::default();

    processor.process(reader, io::stderr());
    if let Err(error) = processor.export_clients(io::stdout()) {
        eprintln!("{}", error);
    }
}

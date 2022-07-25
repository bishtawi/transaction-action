#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]

use std::{
    env,
    fs::File,
    io::{self, BufReader},
};
use transaction_action::Processor;

fn main() {
    let path = env::args()
        .nth(1)
        .expect("First argument should be path to csv file");

    let file = File::open(path).expect("Unable to open file");
    let reader = BufReader::new(file);

    let mut processor = Processor::default();

    processor.process(reader);
    if let Err(error) = processor.print_to_csv(io::stdout()) {
        eprintln!("{}", error);
    }
}

#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]

mod dtos;
mod engine;
mod errors;
mod stores;

use engine::Engine;
use errors::Error;
use std::io::{Read, Write};

type ClientID = u16;
type TransactionID = u32;

#[derive(Default)]
pub struct CSVProcessor {
    engine: Engine,
}

impl CSVProcessor {
    /// Deserializes the reader as a csv and processes each record
    pub fn process(&mut self, csv_input: impl Read, mut err_output: impl Write) {
        let mut csv_reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(csv_input);

        for res in csv_reader.deserialize() {
            if let Err(error) = res
                .map_err(|e| Error::CSVRowReadFailure(e.to_string()))
                .and_then(|r| self.engine.handle(&r))
            {
                if let Err(error_error) = writeln!(err_output, "error: {}", error) {
                    eprintln!("error: {} for error: {}", error_error, error);
                }
            }
        }
    }

    /// Serializes the processed transactions into csv format
    ///
    /// # Errors
    ///
    /// Will return `Err` if the csv writer is unable to write the serialized rows to the passed in writer
    pub fn export_clients(&self, writer: impl Write) -> Result<(), Error> {
        let mut csv_writer = csv::Writer::from_writer(writer);
        csv_writer
            .write_record(&["client", "available", "held", "total", "locked"])
            .map_err(|e| Error::CSVRowWriteFailure(e.to_string()))?;
        for (client_id, client) in self.engine.get_clients() {
            csv_writer
                .write_record(&[
                    client_id.to_string(),
                    client.available_amount.to_string(),
                    client.held_amount.to_string(),
                    (client.available_amount + client.held_amount).to_string(),
                    client.locked.to_string(),
                ])
                .map_err(|e| Error::CSVRowWriteFailure(e.to_string()))?;
        }

        csv_writer
            .flush()
            .map_err(|e| Error::CSVRowWriteFailure(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    // use rust_decimal_macros::dec;
}

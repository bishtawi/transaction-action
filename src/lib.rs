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
    use super::*;
    use rust_decimal::Decimal;
    use rust_decimal_macros::dec;
    use serde::Deserialize;
    use std::{collections::HashMap, fs::File, io::BufReader};

    #[derive(Deserialize, PartialEq, Debug)]
    struct ClientRecord {
        #[serde(rename = "client")]
        id: ClientID,
        available: Decimal,
        held: Decimal,
        total: Decimal,
        locked: bool,
    }

    #[test]
    fn integration_test() {
        let file = File::open("resources/test/test1.csv").expect("Unable to open file");
        let reader = BufReader::new(file);

        let mut processor = CSVProcessor::default();

        let mut err_buffer = Vec::new();
        processor.process(reader, &mut err_buffer);
        let err_msg = String::from_utf8(err_buffer).expect("error logs should be utf8 characters");
        assert_eq!(
            err_msg,
            "error: client 2 cannot withdrawl 3 as available amount is 2.001\n"
        );

        let mut output_buffer = Vec::new();
        processor
            .export_clients(&mut output_buffer)
            .expect("exporting clients should not fail");

        let mut csv_reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(&output_buffer[..]);
        let mut clients = HashMap::new();
        for res in csv_reader.deserialize() {
            let client: ClientRecord =
                res.expect("CSV record needs to deserialize to a ClientRecord");
            assert_eq!(
                client.available + client.held,
                client.total,
                "Available + held should = total"
            );
            assert!(clients.insert(client.id, client).is_none());
        }

        assert_eq!(clients.len(), 2);
        assert_eq!(
            clients[&1],
            ClientRecord {
                id: 1,
                available: dec!(0.5),
                held: dec!(0.0),
                total: dec!(0.5),
                locked: true
            }
        );
        assert_eq!(
            clients[&2],
            ClientRecord {
                id: 2,
                available: dec!(9.9874),
                held: dec!(2.001),
                total: dec!(11.9884),
                locked: false
            }
        );
    }
}

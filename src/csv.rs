use std::io::Read;

use crate::utils::{ClientID, TransactionID};

use anyhow::Result;
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct TransactionRecord {
    #[serde(rename = "type")]
    pub transaction_type: TransactionType,
    #[serde(rename = "client")]
    pub client_id: ClientID,
    #[serde(rename = "tx")]
    pub transaction_id: TransactionID,
    #[serde(deserialize_with = "csv::invalid_option")]
    pub amount: Option<Decimal>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

pub fn process(reader: impl Read) -> Result<()> {
    let mut csv_reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(reader);

    for res in csv_reader.deserialize() {
        let record: TransactionRecord = res?;
        println!("{:?}", record);
    }

    Ok(())
}

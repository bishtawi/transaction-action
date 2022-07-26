use crate::{ClientID, TransactionID};
use rust_decimal::Decimal;
use serde::Deserialize;

/* Would have liked to have done something like:

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransactionRecord {
    Deposit {client: ClientID, tx: TransactionID, amount: Decimal},
    Withdrawal {client: ClientID, tx: TransactionID, amount: Decimal},
    Dispute {client: ClientID, tx: TransactionID},
    Resolve {client: ClientID, tx: TransactionID},
    Chargeback {client: ClientID, tx: TransactionID},
}

Unfortunately, the CSV crate does not support internally tagged enums */

#[derive(Deserialize)]
pub struct TransactionRecord {
    #[serde(rename = "type")]
    pub(crate) transaction_type: TransactionType,
    #[serde(rename = "client")]
    pub(crate) client_id: ClientID,
    #[serde(rename = "tx")]
    pub(crate) transaction_id: TransactionID,
    #[serde(deserialize_with = "csv::invalid_option")]
    pub(crate) amount: Option<Decimal>,
}

#[derive(Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

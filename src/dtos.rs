use rust_decimal::Decimal;
use serde::Deserialize;

pub type ClientID = u16;
pub type TransactionID = u32;

/* Would have liked to have done something like:

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TransactionRecord {
    Deposit {client: ClientID, tx: TransactionID, amount: Decimal},
    Withdrawal {client: ClientID, tx: TransactionID, amount: Decimal},
    Dispute {client: ClientID, tx: TransactionID},
    Resolve {client: ClientID, tx: TransactionID},
    Chargeback {client: ClientID, tx: TransactionID},
}

Unfortunately, the CSV crate does not support internally tagged enums
 */

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

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

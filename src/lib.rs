#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]

mod stores;

use crate::stores::{
    clients::Clients,
    transactions::{self, Transactions},
};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::io::{Read, Write};
use thiserror::Error;

type ClientID = u16;
type TransactionID = u32;

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
pub(crate) struct TransactionRecord {
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
pub(crate) enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("csv row parsing failure: {0}")]
    CSVRowReadFailure(String),
    #[error("csv row writing failure: {0}")]
    CSVRowWriteFailure(String),
    #[error("client {0} is locked")]
    ClientLocked(ClientID),
    #[error("client {0} not exist")]
    ClientNotExist(ClientID),
    #[error("client {id} cannot withdrawl {amount} as available amount is {available}")]
    ClientCannotWithdrawl {
        id: ClientID,
        amount: Decimal,
        available: Decimal,
    },
    #[error("client {id} cannot dispute {amount} as available amount is {available}")]
    ClientCannotDispute {
        id: ClientID,
        amount: Decimal,
        available: Decimal,
    },
    #[error("client {id} cannot resolve {amount} as held amount is {held}")]
    ClientCannotResolve {
        id: ClientID,
        amount: Decimal,
        held: Decimal,
    },
    #[error("client {id} cannot chargeback {amount} as held amount is {held}")]
    ClientCannotChargeBack {
        id: ClientID,
        amount: Decimal,
        held: Decimal,
    },
    #[error("transaction {0} already exist")]
    TransactionIdAlreadyExists(TransactionID),
    #[error("transaction {0} not exist")]
    TransactionNotExists(TransactionID),
    #[error("transaction {0} is not for client {1}")]
    TransactionWithWrongClientId(TransactionID, ClientID),
    #[error("deposit transaction {0} missing amount field")]
    DepositTransactionMissingAmount(TransactionID),
    #[error("withdrawal transaction {0} missing amount field")]
    WithdrawalTransactionMissingAmount(TransactionID),
    #[error("transaction {0} is already in dispute")]
    DisputeAlreadyDisputedTransaction(TransactionID),
    #[error("transaction {0} cannot be disputed as it is not a deposit")]
    DisputeNonDepositTransaction(TransactionID),
    #[error("transaction {0} cannot be resolved as it is not in dispute")]
    ResolveNonDisputedTransaction(TransactionID),
    #[error("transaction {0} cannot be chargebacked as it is not in dispute")]
    ChargeBackNonDisputedTransaction(TransactionID),
}

#[derive(Default)]
pub struct Processor {
    clients_store: Clients,
    transactions_store: Transactions,
}

impl Processor {
    /// Deserializes the reader as a csv and processes each record
    pub fn process(&mut self, reader: impl Read) {
        let mut csv_reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(reader);

        for res in csv_reader.deserialize() {
            if let Err(error) = res
                .map_err(|e| Error::CSVRowReadFailure(e.to_string()))
                .and_then(|r| self.handle(&r))
            {
                eprintln!("{}", error);
            }
        }
    }

    /// Serializes the processed transactions into csv format
    ///
    /// # Errors
    ///
    /// Will return `Err` if the csv writer is unable to write the serialized rows to the passed in writer
    pub fn print_to_csv(&self, writer: impl Write) -> Result<(), Error> {
        let mut csv_writer = csv::Writer::from_writer(writer);
        csv_writer
            .write_record(&["client", "available", "held", "total", "locked"])
            .map_err(|e| Error::CSVRowWriteFailure(e.to_string()))?;
        for (client_id, client) in self.clients_store.get_all() {
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

    fn handle(&mut self, record: &TransactionRecord) -> Result<(), Error> {
        match record.transaction_type {
            TransactionType::Deposit => self.process_deposit(record),
            TransactionType::Withdrawal => self.process_withdrawal(record),
            TransactionType::Dispute => self.process_dispute(record),
            TransactionType::Resolve => self.process_resolve(record),
            TransactionType::Chargeback => self.process_chargeback(record),
        }
    }

    fn process_deposit(&mut self, record: &TransactionRecord) -> Result<(), Error> {
        assert_eq!(record.transaction_type, TransactionType::Deposit);

        let amount = record.amount.ok_or(Error::DepositTransactionMissingAmount(
            record.transaction_id,
        ))?;

        if self.transactions_store.has_id(record.transaction_id) {
            return Err(Error::TransactionIdAlreadyExists(record.transaction_id));
        }

        self.clients_store.deposit(record.client_id, amount)?;

        self.transactions_store.save_new_transaction(
            record.transaction_id,
            transactions::Transaction {
                kind: transactions::Kind::Deposit,
                client_id: record.client_id,
                amount,
                disputed: false,
            },
        );

        Ok(())
    }

    fn process_withdrawal(&mut self, record: &TransactionRecord) -> Result<(), Error> {
        assert_eq!(record.transaction_type, TransactionType::Withdrawal);

        let amount = record
            .amount
            .ok_or(Error::WithdrawalTransactionMissingAmount(
                record.transaction_id,
            ))?;

        if self.transactions_store.has_id(record.transaction_id) {
            return Err(Error::TransactionIdAlreadyExists(record.transaction_id));
        }

        self.clients_store.withdrawal(record.client_id, amount)?;

        self.transactions_store.save_new_transaction(
            record.transaction_id,
            transactions::Transaction {
                kind: transactions::Kind::Withdrawal,
                client_id: record.client_id,
                amount,
                disputed: false,
            },
        );

        Ok(())
    }

    fn process_dispute(&mut self, record: &TransactionRecord) -> Result<(), Error> {
        assert_eq!(record.transaction_type, TransactionType::Dispute);

        let transaction = self
            .transactions_store
            .get_mut_transaction(record.transaction_id, record.client_id)?;

        if transaction.kind != transactions::Kind::Deposit {
            return Err(Error::DisputeNonDepositTransaction(record.transaction_id));
        }

        if transaction.disputed {
            return Err(Error::DisputeAlreadyDisputedTransaction(
                record.transaction_id,
            ));
        }

        self.clients_store
            .move_to_held(record.client_id, transaction.amount)?;
        transaction.disputed = true;

        Ok(())
    }

    fn process_resolve(&mut self, record: &TransactionRecord) -> Result<(), Error> {
        assert_eq!(record.transaction_type, TransactionType::Resolve);

        let transaction = self
            .transactions_store
            .get_mut_transaction(record.transaction_id, record.client_id)?;

        if !transaction.disputed {
            return Err(Error::ResolveNonDisputedTransaction(record.transaction_id));
        }

        self.clients_store
            .move_to_available(record.client_id, transaction.amount)?;
        transaction.disputed = false;

        Ok(())
    }

    fn process_chargeback(&mut self, record: &TransactionRecord) -> Result<(), Error> {
        assert_eq!(record.transaction_type, TransactionType::Chargeback);

        let transaction = self
            .transactions_store
            .get_mut_transaction(record.transaction_id, record.client_id)?;

        if !transaction.disputed {
            return Err(Error::ChargeBackNonDisputedTransaction(
                record.transaction_id,
            ));
        }

        self.clients_store
            .chargeback(record.client_id, transaction.amount)?;
        transaction.disputed = false;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;

    #[test]
    fn test_process_deposit() -> Result<(), Error> {
        let mut processor = Processor::default();
        let client_id = 1;

        // Should correctly process deposits
        processor.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: 9,
            amount: Some(dec!(1.1)),
        })?;
        processor.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: 10,
            amount: Some(dec!(2)),
        })?;
        processor.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: 11,
            amount: Some(dec!(9)),
        })?;

        assert_eq!(
            processor.clients_store.database[&client_id].available_amount,
            dec!(12.1),
            "sequence of deposits should correctly update available amount"
        );

        // Should return error if amount is missing
        assert_eq!(
            processor.process_deposit(&TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client_id,
                transaction_id: 999,
                amount: None
            }),
            Err(Error::DepositTransactionMissingAmount(999)),
            "deposit transaction requires amount"
        );

        // Should return error if transaction id is reused
        assert_eq!(
            processor.process_deposit(&TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client_id,
                transaction_id: 9,
                amount: Some(dec!(1.1)),
            }),
            Err(Error::TransactionIdAlreadyExists(9)),
            "transaction ids need to be globally unique"
        );

        assert_eq!(
            processor.clients_store.database[&client_id].available_amount,
            dec!(12.1),
            "failed deposits should not update available amount"
        );

        Ok(())
    }

    #[test]
    fn test_process_withdrawal() -> Result<(), Error> {
        let mut processor = Processor::default();

        let client_id = 1;
        processor.clients_store.deposit(client_id, dec!(36.22))?;

        // Should correctly process withdrawals
        processor.process_withdrawal(&TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client_id,
            transaction_id: 10,
            amount: Some(dec!(10.01)),
        })?;
        processor.process_withdrawal(&TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client_id,
            transaction_id: 11,
            amount: Some(dec!(10.01)),
        })?;

        assert_eq!(
            processor.clients_store.database[&client_id].available_amount,
            dec!(16.20),
            "sequence of withdrawals should correctly update available amount"
        );

        // Should return error if amount is more than available
        assert_eq!(
            processor.process_withdrawal(&TransactionRecord {
                transaction_type: TransactionType::Withdrawal,
                client_id,
                transaction_id: 12,
                amount: Some(dec!(1000))
            }),
            Err(Error::ClientCannotWithdrawl {
                id: client_id,
                amount: dec!(1000),
                available: dec!(16.20)
            }),
            "should fail if amount is higher than available amount"
        );

        // Should return error if amount is missing
        assert_eq!(
            processor.process_withdrawal(&TransactionRecord {
                transaction_type: TransactionType::Withdrawal,
                client_id,
                transaction_id: 999,
                amount: None
            }),
            Err(Error::WithdrawalTransactionMissingAmount(999)),
            "should fail if amount is missing"
        );

        // Should return error if transaction id is reused
        assert_eq!(
            processor.process_withdrawal(&TransactionRecord {
                transaction_type: TransactionType::Withdrawal,
                client_id,
                transaction_id: 10,
                amount: Some(dec!(0.1)),
            }),
            Err(Error::TransactionIdAlreadyExists(10)),
            "should fail if transaction id is reused"
        );

        assert_eq!(
            processor.clients_store.database[&client_id].available_amount,
            dec!(16.20),
            "failed withdrawals should not update available amount"
        );

        Ok(())
    }

    #[test]
    fn test_process_dispute() -> Result<(), Error> {
        let mut processor = Processor::default();

        let client_id = 1;
        let transaction_id = 55;

        processor.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id,
            amount: Some(dec!(101.95)),
        })?;

        processor.process_dispute(&TransactionRecord {
            transaction_type: TransactionType::Dispute,
            client_id,
            transaction_id,
            amount: None,
        })?;

        assert_eq!(
            processor.clients_store.database[&client_id].available_amount,
            dec!(0.00),
            "dispute should lower available amount"
        );

        assert_eq!(
            processor.clients_store.database[&client_id].held_amount,
            dec!(101.95),
            "dispute should update held amount"
        );

        assert_eq!(
            processor.process_dispute(&TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id,
                transaction_id,
                amount: None,
            }),
            Err(Error::DisputeAlreadyDisputedTransaction(transaction_id)),
            "should not be able to dispute an already disputed transaction"
        );

        let other_deposit_tx = 66;
        let withdrawal_tx = 67;
        processor.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: other_deposit_tx,
            amount: Some(dec!(200)),
        })?;

        processor.process_withdrawal(&TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client_id,
            transaction_id: withdrawal_tx,
            amount: Some(dec!(100)),
        })?;

        assert_eq!(
            processor.process_dispute(&TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id,
                transaction_id: withdrawal_tx,
                amount: None,
            }),
            Err(Error::DisputeNonDepositTransaction(withdrawal_tx)),
            "cannot dispute withdrawal transactions"
        );

        assert_eq!(
            processor.process_dispute(&TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id,
                transaction_id: other_deposit_tx,
                amount: None,
            }),
            Err(Error::ClientCannotDispute {
                id: client_id,
                amount: dec!(200),
                available: dec!(100)
            }),
            "cannot dispute when available balance is lower than transaction amount"
        );

        assert_eq!(
            processor.process_dispute(&TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id,
                transaction_id: 99999,
                amount: None,
            }),
            Err(Error::TransactionNotExists(99999)),
            "cannot dispute non-existant transaction"
        );

        Ok(())
    }

    #[test]
    fn test_process_resolve() -> Result<(), Error> {
        let mut processor = Processor::default();

        let client_id = 1;
        let transaction_id = 55;

        processor.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id,
            amount: Some(dec!(101.95)),
        })?;

        processor.process_dispute(&TransactionRecord {
            transaction_type: TransactionType::Dispute,
            client_id,
            transaction_id,
            amount: None,
        })?;

        processor.process_resolve(&TransactionRecord {
            transaction_type: TransactionType::Resolve,
            client_id,
            transaction_id,
            amount: None,
        })?;

        assert_eq!(
            processor.clients_store.database[&client_id].available_amount,
            dec!(101.95),
            "resolving a dispute should put back held funds into available"
        );

        assert_eq!(
            processor.clients_store.database[&client_id].held_amount,
            dec!(0),
            "resolving a dispute should put back held funds into available"
        );

        assert_eq!(
            processor.process_resolve(&TransactionRecord {
                transaction_type: TransactionType::Resolve,
                client_id,
                transaction_id,
                amount: None,
            }),
            Err(Error::ResolveNonDisputedTransaction(transaction_id)),
            "should not be able to resolve a non-disputed transaction"
        );

        assert_eq!(
            processor.process_resolve(&TransactionRecord {
                transaction_type: TransactionType::Resolve,
                client_id,
                transaction_id: 9875,
                amount: None,
            }),
            Err(Error::TransactionNotExists(9875)),
            "should not be able to resolve non-existant transaction"
        );

        Ok(())
    }

    #[test]
    fn test_process_chargeback() -> Result<(), Error> {
        let mut processor = Processor::default();

        let client_id = 1;
        let transaction_id = 55;

        processor.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id,
            amount: Some(dec!(101.95)),
        })?;

        processor.process_dispute(&TransactionRecord {
            transaction_type: TransactionType::Dispute,
            client_id,
            transaction_id,
            amount: None,
        })?;

        processor.process_chargeback(&TransactionRecord {
            transaction_type: TransactionType::Chargeback,
            client_id,
            transaction_id,
            amount: None,
        })?;

        assert_eq!(
            processor.clients_store.database[&client_id].available_amount,
            dec!(0),
            "chargeback should not put back held funds into available"
        );

        assert_eq!(
            processor.clients_store.database[&client_id].held_amount,
            dec!(0),
            "chargeback should remove funds from held"
        );

        assert_eq!(
            processor.process_chargeback(&TransactionRecord {
                transaction_type: TransactionType::Chargeback,
                client_id,
                transaction_id,
                amount: None,
            }),
            Err(Error::ChargeBackNonDisputedTransaction(transaction_id)),
            "should not be able to chargeback a non-disputed transaction"
        );

        assert_eq!(
            processor.process_chargeback(&TransactionRecord {
                transaction_type: TransactionType::Chargeback,
                client_id,
                transaction_id: 9875,
                amount: None,
            }),
            Err(Error::TransactionNotExists(9875)),
            "should not be able to chargeback non-existant transaction"
        );

        Ok(())
    }
}

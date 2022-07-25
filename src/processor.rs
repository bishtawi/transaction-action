use std::io::Read;

use crate::{
    dtos::{TransactionRecord, TransactionType},
    stores::{
        clients::Clients,
        transactions::{self, Transactions},
    },
};

use anyhow::{anyhow, bail, Result};

#[derive(Default)]
pub struct Processor {
    clients_store: Clients,
    transactions_store: Transactions,
}

impl Processor {
    pub fn handle(&mut self, reader: impl Read) {
        let mut csv_reader = csv::ReaderBuilder::new()
            .trim(csv::Trim::All)
            .from_reader(reader);

        for res in csv_reader.deserialize() {
            if let Err(error) = res
                .map_err(|e| anyhow!("Failure to parse csv row: {}", e))
                .and_then(|r| self.process(&r))
            {
                eprintln!("{}", error);
            }
        }
    }

    fn process(&mut self, record: &TransactionRecord) -> Result<()> {
        match record.transaction_type {
            TransactionType::Deposit => self.process_deposit(record),
            TransactionType::Withdrawal => self.process_withdrawal(record),
            TransactionType::Dispute => self.process_dispute(record),
            TransactionType::Resolve => self.process_resolve(record),
            TransactionType::Chargeback => self.process_chargeback(record),
        }
    }

    fn process_deposit(&mut self, record: &TransactionRecord) -> Result<()> {
        debug_assert_eq!(record.transaction_type, TransactionType::Deposit);

        let amount = record.amount.ok_or_else(|| {
            anyhow!(
                "Deposit transaction {} missing amount field",
                record.transaction_id
            )
        })?;

        self.transactions_store.save_new_transaction(
            record.transaction_id,
            transactions::Transaction {
                kind: transactions::Kind::Deposit,
                client_id: record.client_id,
                amount,
                disputed: false,
            },
        )?;

        self.clients_store.deposit(record.client_id, amount)
    }

    fn process_withdrawal(&mut self, record: &TransactionRecord) -> Result<()> {
        debug_assert_eq!(record.transaction_type, TransactionType::Withdrawal);

        let amount = record.amount.ok_or_else(|| {
            anyhow!(
                "Withdrawal transaction {} missing amount field",
                record.transaction_id
            )
        })?;

        self.transactions_store.save_new_transaction(
            record.transaction_id,
            transactions::Transaction {
                kind: transactions::Kind::Withdrawal,
                client_id: record.client_id,
                amount,
                disputed: false,
            },
        )?;

        self.clients_store.withdrawal(record.client_id, amount)
    }

    fn process_dispute(&mut self, record: &TransactionRecord) -> Result<()> {
        debug_assert_eq!(record.transaction_type, TransactionType::Dispute);

        let transaction = self
            .transactions_store
            .get_mut_transaction(record.transaction_id, record.client_id)?;

        if transaction.kind != transactions::Kind::Deposit {
            bail!(
                "Attempting to dispute non-deposit transaction {} which is not supported",
                record.transaction_id
            );
        }

        if transaction.disputed {
            bail!(
                "Attempting to dispute already disputed transaction {}",
                record.transaction_id
            );
        }

        self.clients_store
            .move_to_held(record.client_id, transaction.amount)?;
        transaction.disputed = true;

        Ok(())
    }

    fn process_resolve(&mut self, record: &TransactionRecord) -> Result<()> {
        debug_assert_eq!(record.transaction_type, TransactionType::Resolve);

        let transaction = self
            .transactions_store
            .get_mut_transaction(record.transaction_id, record.client_id)?;

        if !transaction.disputed {
            bail!(
                "Attempting to resolve non-disputed transaction {}",
                record.transaction_id
            );
        }

        self.clients_store
            .move_to_available(record.client_id, transaction.amount)?;
        transaction.disputed = false;

        Ok(())
    }

    fn process_chargeback(&mut self, record: &TransactionRecord) -> Result<()> {
        debug_assert_eq!(record.transaction_type, TransactionType::Chargeback);

        let transaction = self
            .transactions_store
            .get_mut_transaction(record.transaction_id, record.client_id)?;

        if !transaction.disputed {
            bail!(
                "Attempting to chargeback non-disputed transaction {}",
                record.transaction_id
            );
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

    use anyhow::Result;
    use rust_decimal_macros::dec;

    #[test]
    fn test_process_deposit() -> Result<()> {
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
            processor
                .clients_store
                .get_client(client_id)?
                .available_amount,
            dec!(12.1),
            "sequence of deposits should correctly update available amount"
        );

        // Should return error if amount is missing
        assert!(
            processor
                .process_deposit(&TransactionRecord {
                    transaction_type: TransactionType::Deposit,
                    client_id,
                    transaction_id: 999,
                    amount: None
                })
                .is_err(),
            "deposit transaction requires amount"
        );

        // Should return error if transaction id is reused
        assert!(
            processor
                .process_deposit(&TransactionRecord {
                    transaction_type: TransactionType::Deposit,
                    client_id,
                    transaction_id: 9,
                    amount: Some(dec!(1.1)),
                })
                .is_err(),
            "transaction ids need to be globally unique"
        );

        assert_eq!(
            processor
                .clients_store
                .get_client(client_id)?
                .available_amount,
            dec!(12.1),
            "failed deposits should not update available amount"
        );

        Ok(())
    }

    #[test]
    fn test_process_withdrawal() -> Result<()> {
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
            processor
                .clients_store
                .get_client(client_id)?
                .available_amount,
            dec!(16.20),
            "sequence of withdrawals should correctly update available amount"
        );

        // Should return error if amount is more than available
        assert!(
            processor
                .process_withdrawal(&TransactionRecord {
                    transaction_type: TransactionType::Withdrawal,
                    client_id,
                    transaction_id: 12,
                    amount: Some(dec!(1000))
                })
                .is_err(),
            "should fail if amount is higher than available amount"
        );

        // Should return error if amount is missing
        assert!(
            processor
                .process_withdrawal(&TransactionRecord {
                    transaction_type: TransactionType::Withdrawal,
                    client_id,
                    transaction_id: 999,
                    amount: None
                })
                .is_err(),
            "should fail if amount is missing"
        );

        // Should return error if transaction id is reused
        assert!(
            processor
                .process_withdrawal(&TransactionRecord {
                    transaction_type: TransactionType::Withdrawal,
                    client_id,
                    transaction_id: 10,
                    amount: Some(dec!(0.1)),
                })
                .is_err(),
            "should fail if transaction id is reused"
        );

        assert_eq!(
            processor
                .clients_store
                .get_client(client_id)?
                .available_amount,
            dec!(16.20),
            "failed withdrawals should not update available amount"
        );

        Ok(())
    }

    #[test]
    fn test_process_dispute() -> Result<()> {
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
            processor
                .clients_store
                .get_client(client_id)?
                .available_amount,
            dec!(0.00),
            "dispute should lower available amount"
        );

        assert_eq!(
            processor.clients_store.get_client(client_id)?.held_amount,
            dec!(101.95),
            "dispute should update held amount"
        );

        assert!(
            processor
                .process_dispute(&TransactionRecord {
                    transaction_type: TransactionType::Dispute,
                    client_id,
                    transaction_id,
                    amount: None,
                })
                .is_err(),
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

        assert!(
            processor
                .process_dispute(&TransactionRecord {
                    transaction_type: TransactionType::Dispute,
                    client_id,
                    transaction_id: withdrawal_tx,
                    amount: None,
                })
                .is_err(),
            "cannot dispute withdrawal transactions"
        );

        assert!(
            processor
                .process_dispute(&TransactionRecord {
                    transaction_type: TransactionType::Dispute,
                    client_id,
                    transaction_id: other_deposit_tx,
                    amount: None,
                })
                .is_err(),
            "cannot dispute when available balance is lower than transaction amount"
        );

        assert!(
            processor
                .process_dispute(&TransactionRecord {
                    transaction_type: TransactionType::Dispute,
                    client_id,
                    transaction_id: 99999,
                    amount: None,
                })
                .is_err(),
            "cannot dispute non-existant transaction"
        );

        Ok(())
    }

    #[test]
    fn test_process_resolve() -> Result<()> {
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
            processor
                .clients_store
                .get_client(client_id)?
                .available_amount,
            dec!(101.95),
            "resolving a dispute should put back held funds into available"
        );

        assert_eq!(
            processor.clients_store.get_client(client_id)?.held_amount,
            dec!(0),
            "resolving a dispute should put back held funds into available"
        );

        assert!(
            processor
                .process_resolve(&TransactionRecord {
                    transaction_type: TransactionType::Resolve,
                    client_id,
                    transaction_id,
                    amount: None,
                })
                .is_err(),
            "should not be able to resolve a non-disputed transaction"
        );

        assert!(
            processor
                .process_resolve(&TransactionRecord {
                    transaction_type: TransactionType::Resolve,
                    client_id,
                    transaction_id: 9875,
                    amount: None,
                })
                .is_err(),
            "should not be able to resolve non-existant transaction"
        );

        Ok(())
    }

    #[test]
    fn test_process_chargeback() -> Result<()> {
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
            processor
                .clients_store
                .get_client(client_id)?
                .available_amount,
            dec!(0),
            "chargeback should not put back held funds into available"
        );

        assert_eq!(
            processor.clients_store.get_client(client_id)?.held_amount,
            dec!(0),
            "chargeback should remove funds from held"
        );

        assert!(
            processor
                .process_chargeback(&TransactionRecord {
                    transaction_type: TransactionType::Chargeback,
                    client_id,
                    transaction_id,
                    amount: None,
                })
                .is_err(),
            "should not be able to chargeback a non-disputed transaction"
        );

        assert!(
            processor
                .process_chargeback(&TransactionRecord {
                    transaction_type: TransactionType::Chargeback,
                    client_id,
                    transaction_id: 9875,
                    amount: None,
                })
                .is_err(),
            "should not be able to chargeback non-existant transaction"
        );

        Ok(())
    }
}

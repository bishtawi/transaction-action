use std::collections::HashMap;

use crate::stores::{
    clients::{Client, Clients},
    transactions::{self, Transactions},
};
use crate::{
    dtos::{TransactionRecord, TransactionType},
    errors::Error,
    ClientID,
};

#[derive(Default)]
pub struct Engine {
    clients_store: Clients,
    transactions_store: Transactions,
}

impl Engine {
    pub fn handle(&mut self, record: &TransactionRecord) -> Result<(), Error> {
        match record.transaction_type {
            TransactionType::Deposit => self.process_deposit(record),
            TransactionType::Withdrawal => self.process_withdrawal(record),
            TransactionType::Dispute => self.process_dispute(record),
            TransactionType::Resolve => self.process_resolve(record),
            TransactionType::Chargeback => self.process_chargeback(record),
        }
    }

    pub(crate) fn get_clients(&self) -> &HashMap<ClientID, Client> {
        self.clients_store.get_all()
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
        let mut engine = Engine::default();
        let client_id = 1;

        // Should correctly process deposits
        engine.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: 9,
            amount: Some(dec!(1.1)),
        })?;
        engine.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: 10,
            amount: Some(dec!(2)),
        })?;
        engine.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: 11,
            amount: Some(dec!(9)),
        })?;

        assert_eq!(
            engine.clients_store.database[&client_id].available_amount,
            dec!(12.1),
            "sequence of deposits should correctly update available amount"
        );

        // Should return error if amount is missing
        assert_eq!(
            engine.process_deposit(&TransactionRecord {
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
            engine.process_deposit(&TransactionRecord {
                transaction_type: TransactionType::Deposit,
                client_id,
                transaction_id: 9,
                amount: Some(dec!(1.1)),
            }),
            Err(Error::TransactionIdAlreadyExists(9)),
            "transaction ids need to be globally unique"
        );

        assert_eq!(
            engine.clients_store.database[&client_id].available_amount,
            dec!(12.1),
            "failed deposits should not update available amount"
        );

        Ok(())
    }

    #[test]
    fn test_process_withdrawal() -> Result<(), Error> {
        let mut engine = Engine::default();

        let client_id = 1;
        engine.clients_store.deposit(client_id, dec!(36.22))?;

        // Should correctly process withdrawals
        engine.process_withdrawal(&TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client_id,
            transaction_id: 10,
            amount: Some(dec!(10.01)),
        })?;
        engine.process_withdrawal(&TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client_id,
            transaction_id: 11,
            amount: Some(dec!(10.01)),
        })?;

        assert_eq!(
            engine.clients_store.database[&client_id].available_amount,
            dec!(16.20),
            "sequence of withdrawals should correctly update available amount"
        );

        // Should return error if amount is more than available
        assert_eq!(
            engine.process_withdrawal(&TransactionRecord {
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
            engine.process_withdrawal(&TransactionRecord {
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
            engine.process_withdrawal(&TransactionRecord {
                transaction_type: TransactionType::Withdrawal,
                client_id,
                transaction_id: 10,
                amount: Some(dec!(0.1)),
            }),
            Err(Error::TransactionIdAlreadyExists(10)),
            "should fail if transaction id is reused"
        );

        assert_eq!(
            engine.clients_store.database[&client_id].available_amount,
            dec!(16.20),
            "failed withdrawals should not update available amount"
        );

        Ok(())
    }

    #[test]
    fn test_process_dispute() -> Result<(), Error> {
        let mut engine = Engine::default();

        let client_id = 1;
        let transaction_id = 55;

        engine.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id,
            amount: Some(dec!(101.95)),
        })?;

        engine.process_dispute(&TransactionRecord {
            transaction_type: TransactionType::Dispute,
            client_id,
            transaction_id,
            amount: None,
        })?;

        assert_eq!(
            engine.clients_store.database[&client_id].available_amount,
            dec!(0.00),
            "dispute should lower available amount"
        );

        assert_eq!(
            engine.clients_store.database[&client_id].held_amount,
            dec!(101.95),
            "dispute should update held amount"
        );

        assert_eq!(
            engine.process_dispute(&TransactionRecord {
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
        engine.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id: other_deposit_tx,
            amount: Some(dec!(200)),
        })?;

        engine.process_withdrawal(&TransactionRecord {
            transaction_type: TransactionType::Withdrawal,
            client_id,
            transaction_id: withdrawal_tx,
            amount: Some(dec!(100)),
        })?;

        assert_eq!(
            engine.process_dispute(&TransactionRecord {
                transaction_type: TransactionType::Dispute,
                client_id,
                transaction_id: withdrawal_tx,
                amount: None,
            }),
            Err(Error::DisputeNonDepositTransaction(withdrawal_tx)),
            "cannot dispute withdrawal transactions"
        );

        assert_eq!(
            engine.process_dispute(&TransactionRecord {
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
            engine.process_dispute(&TransactionRecord {
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
        let mut engine = Engine::default();

        let client_id = 1;
        let transaction_id = 55;

        engine.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id,
            amount: Some(dec!(101.95)),
        })?;

        engine.process_dispute(&TransactionRecord {
            transaction_type: TransactionType::Dispute,
            client_id,
            transaction_id,
            amount: None,
        })?;

        engine.process_resolve(&TransactionRecord {
            transaction_type: TransactionType::Resolve,
            client_id,
            transaction_id,
            amount: None,
        })?;

        assert_eq!(
            engine.clients_store.database[&client_id].available_amount,
            dec!(101.95),
            "resolving a dispute should put back held funds into available"
        );

        assert_eq!(
            engine.clients_store.database[&client_id].held_amount,
            dec!(0),
            "resolving a dispute should put back held funds into available"
        );

        assert_eq!(
            engine.process_resolve(&TransactionRecord {
                transaction_type: TransactionType::Resolve,
                client_id,
                transaction_id,
                amount: None,
            }),
            Err(Error::ResolveNonDisputedTransaction(transaction_id)),
            "should not be able to resolve a non-disputed transaction"
        );

        assert_eq!(
            engine.process_resolve(&TransactionRecord {
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
        let mut engine = Engine::default();

        let client_id = 1;
        let transaction_id = 55;

        engine.process_deposit(&TransactionRecord {
            transaction_type: TransactionType::Deposit,
            client_id,
            transaction_id,
            amount: Some(dec!(101.95)),
        })?;

        engine.process_dispute(&TransactionRecord {
            transaction_type: TransactionType::Dispute,
            client_id,
            transaction_id,
            amount: None,
        })?;

        engine.process_chargeback(&TransactionRecord {
            transaction_type: TransactionType::Chargeback,
            client_id,
            transaction_id,
            amount: None,
        })?;

        assert_eq!(
            engine.clients_store.database[&client_id].available_amount,
            dec!(0),
            "chargeback should not put back held funds into available"
        );

        assert_eq!(
            engine.clients_store.database[&client_id].held_amount,
            dec!(0),
            "chargeback should remove funds from held"
        );

        assert_eq!(
            engine.process_chargeback(&TransactionRecord {
                transaction_type: TransactionType::Chargeback,
                client_id,
                transaction_id,
                amount: None,
            }),
            Err(Error::ChargeBackNonDisputedTransaction(transaction_id)),
            "should not be able to chargeback a non-disputed transaction"
        );

        assert_eq!(
            engine.process_chargeback(&TransactionRecord {
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

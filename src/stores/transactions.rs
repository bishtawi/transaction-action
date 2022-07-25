use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use rust_decimal::Decimal;

use crate::dtos::{ClientID, TransactionID};

// DAO (representation of what would be our Transactions table in the database)
pub struct Transaction {
    pub kind: Kind,
    pub client_id: ClientID,
    pub amount: Decimal,
    pub disputed: bool,
}

#[derive(PartialEq, Eq)]
pub enum Kind {
    Deposit,
    Withdrawal,
}

// In a proper implementation, the Transactions store would connect to a database instead of being an in-memory store
#[derive(Default)]
pub struct Transactions {
    database: HashMap<TransactionID, Transaction>,
}

impl Transactions {
    pub fn save_new_transaction(
        &mut self,
        transaction_id: TransactionID,
        transaction: Transaction,
    ) -> Result<()> {
        if self.database.contains_key(&transaction_id) {
            bail!(
                "Transaction id {} already exists in database",
                transaction_id
            );
        }

        let previous = self.database.insert(transaction_id, transaction);
        debug_assert!(previous.is_none());

        Ok(())
    }

    pub fn get_transaction(
        &self,
        transaction_id: TransactionID,
        client_id: ClientID,
    ) -> Result<&Transaction> {
        let transaction = self
            .database
            .get(&transaction_id)
            .ok_or_else(|| anyhow!("Transaction {} does not exist", transaction_id))?;
        if transaction.client_id != client_id {
            bail!(
                "Transaction {} is not for client {}",
                transaction_id,
                client_id
            );
        }

        Ok(transaction)
    }

    pub fn get_mut_transaction(
        &mut self,
        transaction_id: TransactionID,
        client_id: ClientID,
    ) -> Result<&mut Transaction> {
        let transaction = self
            .database
            .get_mut(&transaction_id)
            .ok_or_else(|| anyhow!("Transaction {} does not exist", transaction_id))?;
        if transaction.client_id != client_id {
            bail!(
                "Transaction {} is not for client {}",
                transaction_id,
                client_id
            );
        }

        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;
    use rust_decimal_macros::dec;

    #[test]
    fn test_save_new_transaction() -> Result<()> {
        let mut transactions = Transactions::default();
        assert_eq!(
            transactions.database.len(),
            0,
            "new datastore should be empty"
        );

        let transaction_id = 445;
        let client_id = 12;

        // Should save new transaction
        transactions.save_new_transaction(
            transaction_id,
            Transaction {
                kind: Kind::Deposit,
                client_id,
                amount: dec!(45),
                disputed: false,
            },
        )?;
        assert_eq!(
            transactions.database.len(),
            1,
            "Saving new transaction should create new entry in database"
        );
        assert_eq!(
            transactions.database[&transaction_id].amount,
            dec!(45),
            "transaction amount should match"
        );

        // Should fail to save new transaction with duplicate id
        assert!(
            transactions
                .save_new_transaction(
                    transaction_id,
                    Transaction {
                        kind: Kind::Withdrawal,
                        client_id,
                        amount: dec!(12),
                        disputed: true
                    }
                )
                .is_err(),
            "should fail to save due to duplicate id"
        );
        assert_eq!(transactions.database.len(), 1, "database should not grow");
        assert_eq!(
            transactions.database[&transaction_id].amount,
            dec!(45),
            "amount of existing transaction should not change"
        );
        assert!(
            !transactions.database[&transaction_id].disputed,
            "existing transaction disputed flag should not change"
        );

        Ok(())
    }

    #[test]
    fn test_get_transaction() -> Result<()> {
        let mut transactions = Transactions::default();

        let transaction_id = 445;
        let other_transaction_id = 446;
        let client_id = 12;
        let other_client_id = 13;
        transactions.database.insert(
            transaction_id,
            Transaction {
                kind: Kind::Withdrawal,
                client_id,
                amount: dec!(12),
                disputed: true,
            },
        );

        // Should successfully get transaction with matching transaction id and client id
        let transaction = transactions.get_transaction(transaction_id, client_id)?;
        assert_eq!(transaction.client_id, client_id, "client id should match");
        assert_eq!(
            transaction.amount,
            dec!(12),
            "transaction amount should match"
        );

        // Error cases
        assert!(
            transactions
                .get_mut_transaction(other_transaction_id, client_id)
                .is_err(),
            "unrecognized transaction id should be an error"
        );
        assert!(
            transactions
                .get_mut_transaction(transaction_id, other_client_id)
                .is_err(),
            "transaction id with mismatched client id should be an error"
        );

        Ok(())
    }

    #[test]
    fn test_get_mut_transaction() -> Result<()> {
        let mut transactions = Transactions::default();

        let transaction_id = 445;
        let other_transaction_id = 446;
        let client_id = 12;
        let other_client_id = 13;
        transactions.database.insert(
            transaction_id,
            Transaction {
                kind: Kind::Withdrawal,
                client_id,
                amount: dec!(12),
                disputed: true,
            },
        );

        // Should successfully get transaction with matching transaction id and client id
        let transaction = transactions.get_mut_transaction(transaction_id, client_id)?;
        assert_eq!(transaction.client_id, client_id, "client id should match");
        assert_eq!(
            transaction.amount,
            dec!(12),
            "transaction amount should match"
        );
        transaction.disputed = true;

        // Error cases
        assert!(
            transactions
                .get_mut_transaction(other_transaction_id, client_id)
                .is_err(),
            "unrecognized transaction id should be an error"
        );
        assert!(
            transactions
                .get_mut_transaction(transaction_id, other_client_id)
                .is_err(),
            "transaction id with mismatched client id should be an error"
        );

        Ok(())
    }
}

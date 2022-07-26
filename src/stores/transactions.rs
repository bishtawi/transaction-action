use crate::{errors::Error, ClientID, TransactionID};
use rust_decimal::Decimal;
use std::collections::HashMap;

// DAO (representation of what would be our Transactions table in the database)
#[derive(Debug)]
pub(crate) struct Transaction {
    pub(crate) kind: Kind,
    pub(crate) client_id: ClientID,
    pub(crate) amount: Decimal,
    pub(crate) disputed: bool,
}

#[derive(PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    Deposit,
    Withdrawal,
}

// In a proper implementation, the Transactions store would connect to a database instead of being an in-memory store
#[derive(Default)]
pub(crate) struct Transactions {
    database: HashMap<TransactionID, Transaction>,
}

impl Transactions {
    pub(crate) fn has_id(&self, transaction_id: TransactionID) -> bool {
        self.database.contains_key(&transaction_id)
    }

    pub(crate) fn save_new_transaction(
        &mut self,
        transaction_id: TransactionID,
        transaction: Transaction,
    ) {
        let prev = self.database.insert(transaction_id, transaction);
        assert!(prev.is_none());
    }

    pub(crate) fn get_mut_transaction(
        &mut self,
        transaction_id: TransactionID,
        client_id: ClientID,
    ) -> Result<&mut Transaction, Error> {
        let transaction = self
            .database
            .get_mut(&transaction_id)
            .ok_or(Error::TransactionNotExists(transaction_id))?;
        if transaction.client_id != client_id {
            return Err(Error::TransactionWithWrongClientId(
                transaction_id,
                client_id,
            ));
        }

        Ok(transaction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;

    #[test]
    fn test_has_id() {
        let mut transactions = Transactions::default();

        let transaction_id = 445;
        let other_transaction_id = 446;
        transactions.database.insert(
            transaction_id,
            Transaction {
                kind: Kind::Withdrawal,
                client_id: 4,
                amount: dec!(12),
                disputed: true,
            },
        );

        assert!(transactions.has_id(transaction_id));
        assert!(!transactions.has_id(other_transaction_id));
    }

    #[test]
    fn test_save_new_transaction() {
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
        );
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
    }

    #[test]
    fn test_get_mut_transaction() -> Result<(), Error> {
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
        assert_eq!(
            transactions
                .get_mut_transaction(other_transaction_id, client_id)
                .unwrap_err(),
            Error::TransactionNotExists(other_transaction_id),
            "unrecognized transaction id should be an error"
        );
        assert_eq!(
            transactions
                .get_mut_transaction(transaction_id, other_client_id)
                .unwrap_err(),
            Error::TransactionWithWrongClientId(transaction_id, other_client_id),
            "transaction id with mismatched client id should be an error"
        );

        Ok(())
    }
}

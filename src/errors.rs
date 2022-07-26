use crate::{ClientID, TransactionID};
use rust_decimal::Decimal;
use thiserror::Error;

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

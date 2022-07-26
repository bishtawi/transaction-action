# Transaction-action project

Given a csv of transactions (deposit, withdrawal, dispute, resolve and chargeback types), this tool will process all transactions and return a csv of the client account balances.

[![CI status](https://github.com/bishtawi/transaction-action/workflows/test/badge.svg)](https://github.com/bishtawi/transaction-action/actions/workflows/test.yml)

## Assumptions

- Only deposit transactions can be disputed
    - The requirements for disputing/resolving a transaction give the impression that only deposit transaction can be disputed
    - The disputing/resolving requirements just dont make sense for withdrawal transactions
- Disputing transactions are rejected if there are not enough available funds to move to held
- Resolving transactions are rejected if there are not enough held funds to move to available
- Chargeback transactions are rejected if there are not enough held funds
- Disputing/resolving/chargeback transactions are rejected if the client id does not match the corresponding deposit transaction client id
- Deposit/withdrawal transactions are rejected if their transaction id has already been seen
- When a client is "locked", all future transactions with their client id are rejected

## Design

This Rust project contains both a binary (fully contained in `src/main.rs`) and a library (the rest of the `src/*.rs` files starting with `src/lib.rs`).

The binary only depends on the stdlib and of course this library (plus all of this library's dependencies) as the binary basically wraps the library into a CLI tool.

The library is broken up into three layers: `CSVProcessor`, `Engine` and stores (`Clients` and `Transactions`).

1. CSVProcessor: Handles parsing the `std::io::Read` object as a CSV, then iterates through each `TransactionRecord` of the CSV and passes it to the `Engine` for processing.
2. Engine: The transaction engine that processes the incoming `TransactionRecord` by updating the datastores.
3. Stores (Clients and Transactions): Encapsulates the client and transaction databases (currently implemented as in-memory datastores).

The CSV input to the CSVProcessor only needs to implement `std::io::Read`, allowing the CSV data to come from more places than just a CSV file.

CSVProcessor and Engine are separate to decouple the CSV parsing from the transaction business logic as future uses of the transaction engine might not be CSV related (HTTP REST API for example).

Likewise, the transaction engine is separated from the stores for similar decoupling reasons.

## Usage

```
cargo run -- path/to/transactions.csv > accounts.csv
```

The output csv (summary of client balances) is written to stdout. Errors/warnings are written to stderr.

## CI

Github Actions are set up to run linting, unit tests, and a security audit on the codebase. You can view the results [here](https://github.com/bishtawi/transaction-action/actions/workflows/test.yml).

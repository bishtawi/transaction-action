# Transaction-action project

Given a csv of transactions (deposit, withdrawal, dispute, resolve and chargeback), this tool will process all transactions and return a csv of the client account balances.

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

## Future improvements

Given more time, the obvious improvement to make would be to replace the in-memory datastores with a real database (Postgres for example)

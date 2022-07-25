use crate::{ClientID, Error};
use rust_decimal::Decimal;
use std::collections::HashMap;

// DAO (representation of what would be our Clients table in the database)
#[derive(Default)]
pub(crate) struct Client {
    pub(crate) available_amount: Decimal,
    pub(crate) held_amount: Decimal,
    pub(crate) locked: bool,
}

// In a proper implementation, the Clients store would connect to a database instead of being an in-memory store
#[derive(Default)]
pub(crate) struct Clients {
    pub(crate) database: HashMap<ClientID, Client>,
}

impl Clients {
    pub(crate) fn deposit(&mut self, id: ClientID, amount: Decimal) -> Result<(), Error> {
        let client = self.database.entry(id).or_insert_with(Client::default);
        if client.locked {
            return Err(Error::ClientLocked(id));
        }

        client.available_amount += amount;

        Ok(())
    }

    pub(crate) fn withdrawal(&mut self, id: ClientID, amount: Decimal) -> Result<(), Error> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or(Error::ClientNotExist(id))?;
        if client.locked {
            return Err(Error::ClientLocked(id));
        }

        let new_amount = client.available_amount - amount;
        if new_amount.is_sign_negative() {
            return Err(Error::ClientCannotWithdrawl {
                id,
                amount,
                available: client.available_amount,
            });
        }

        client.available_amount = new_amount;

        Ok(())
    }

    pub(crate) fn move_to_held(&mut self, id: ClientID, amount: Decimal) -> Result<(), Error> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or(Error::ClientNotExist(id))?;
        if client.locked {
            return Err(Error::ClientLocked(id));
        }

        let new_available = client.available_amount - amount;
        if new_available.is_sign_negative() {
            return Err(Error::ClientCannotDispute {
                id,
                amount,
                available: client.available_amount,
            });
        }

        client.available_amount = new_available;
        client.held_amount += amount;

        Ok(())
    }

    pub(crate) fn move_to_available(&mut self, id: ClientID, amount: Decimal) -> Result<(), Error> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or(Error::ClientNotExist(id))?;
        if client.locked {
            return Err(Error::ClientLocked(id));
        }

        let new_held = client.held_amount - amount;
        if new_held.is_sign_negative() {
            return Err(Error::ClientCannotResolve {
                id,
                amount,
                held: client.held_amount,
            });
        }

        client.held_amount = new_held;
        client.available_amount += amount;

        Ok(())
    }

    pub(crate) fn chargeback(&mut self, id: ClientID, amount: Decimal) -> Result<(), Error> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or(Error::ClientNotExist(id))?;
        if client.locked {
            return Err(Error::ClientLocked(id));
        }

        client.locked = true;

        let new_held = client.held_amount - amount;
        if new_held.is_sign_negative() {
            return Err(Error::ClientCannotChargeBack {
                id,
                amount,
                held: client.held_amount,
            });
        }

        client.held_amount = new_held;
        Ok(())
    }

    pub(crate) fn get_all(&self) -> &HashMap<ClientID, Client> {
        &self.database
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rust_decimal_macros::dec;

    #[test]
    fn test_deposit() -> Result<(), Error> {
        let mut clients = Clients::default();
        assert_eq!(
            clients.database.len(),
            0,
            "default client database should be empty"
        );

        let client_id = 123;
        let other_client_id = 321;

        // Should create new client with correct available amount, zero held and not locked
        clients.deposit(client_id, dec!(222.12))?;
        assert_eq!(
            clients.database.len(),
            1,
            "one new client should be created"
        );
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(222.12),
            "new client should have deposited amount"
        );
        assert_eq!(
            clients.database[&client_id].held_amount,
            dec!(0),
            "new client should have no held amount"
        );
        assert!(
            !clients.database[&client_id].locked,
            "new client should not be locked"
        );

        // Should update available amount
        clients.deposit(client_id, dec!(95))?;
        assert_eq!(clients.database.len(), 1, "should update existing client");
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(317.12),
            "client available amount should be updated with new deposit"
        );
        assert_eq!(
            clients.database[&client_id].held_amount,
            dec!(0),
            "client held amount should be unchanged"
        );
        assert!(
            !clients.database[&client_id].locked,
            "client should still be unlocked"
        );

        // Should create new client with correct available amount, without modifying previous client
        clients.deposit(other_client_id, dec!(0.22))?;
        assert_eq!(clients.database.len(), 2, "new client should be created");
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(317.12),
            "existing client amount should be untouched"
        );
        assert_eq!(
            clients.database[&other_client_id].available_amount,
            dec!(0.22),
            "new client should have deposited amount"
        );

        // Should fail to deposit money if client is locked
        clients.database.get_mut(&other_client_id).unwrap().locked = true;
        assert_eq!(
            clients.deposit(other_client_id, dec!(1)),
            Err(Error::ClientLocked(other_client_id)),
            "should fail to deposit money as client is locked"
        );
        assert_eq!(
            clients.database[&other_client_id].available_amount,
            dec!(0.22),
            "available amount should be unchanged"
        );

        Ok(())
    }

    #[test]
    fn test_withdrawal() -> Result<(), Error> {
        let mut clients = Clients::default();

        let client_id = 111;
        let other_client_id = 112;
        let locked_client_id = 113;
        let non_exist_client_id = 114;
        clients.database.insert(
            client_id,
            Client {
                available_amount: dec!(95.6),
                held_amount: dec!(0),
                locked: false,
            },
        );
        clients.database.insert(
            other_client_id,
            Client {
                available_amount: dec!(0.6),
                held_amount: dec!(0),
                locked: false,
            },
        );
        clients.database.insert(
            locked_client_id,
            Client {
                available_amount: dec!(100),
                held_amount: dec!(0),
                locked: true,
            },
        );

        // Can withdrawl without touching another client
        clients.withdrawal(client_id, dec!(0.6))?;
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(95),
            "withdrawal should decrease available amount"
        );
        assert_eq!(
            clients.database[&other_client_id].available_amount,
            dec!(0.6),
            "withdrawal shouldnt touch other clients"
        );
        assert_eq!(
            clients.database[&locked_client_id].available_amount,
            dec!(100),
            "withdrawal shouldnt touch other clients"
        );

        // Cannot withdrawl higher than available amount
        assert_eq!(
            clients.withdrawal(other_client_id, dec!(1)),
            Err(Error::ClientCannotWithdrawl {
                id: other_client_id,
                amount: dec!(1),
                available: dec!(0.6)
            }),
            "should fail to withdrawl higher than available amount"
        );
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(95),
            "available amount should be unmodified"
        );

        // Cannot withdrawl from locked account
        assert_eq!(
            clients.withdrawal(locked_client_id, dec!(1)),
            Err(Error::ClientLocked(locked_client_id)),
            "should fail to withdrawl from locked account"
        );
        assert_eq!(
            clients.database[&locked_client_id].available_amount,
            dec!(100),
            "available amount should be unmodified"
        );

        // Cannot withdrawl from non-existant account
        assert_eq!(
            clients.withdrawal(non_exist_client_id, dec!(1)),
            Err(Error::ClientNotExist(non_exist_client_id)),
            "should fail to withdrawl from non existant account"
        );

        Ok(())
    }

    #[test]
    fn test_move_to_held() -> Result<(), Error> {
        let mut clients = Clients::default();

        let client_id = 94;
        let other_client_id = 95;
        let locked_client_id = 96;
        clients.database.insert(
            client_id,
            Client {
                available_amount: dec!(95.6),
                held_amount: dec!(131.33),
                locked: false,
            },
        );
        clients.database.insert(
            other_client_id,
            Client {
                available_amount: dec!(12),
                held_amount: dec!(31),
                locked: false,
            },
        );
        clients.database.insert(
            locked_client_id,
            Client {
                available_amount: dec!(100),
                held_amount: dec!(11),
                locked: true,
            },
        );

        // Should correctly move funds from available to held
        clients.move_to_held(client_id, dec!(30))?;
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(65.6),
            "client available amount should correctly decrease"
        );
        assert_eq!(
            clients.database[&client_id].held_amount,
            dec!(161.33),
            "client held amount should correctly increase"
        );

        // Should fail to move more funds than available
        assert_eq!(
            clients.move_to_held(other_client_id, dec!(12.01)),
            Err(Error::ClientCannotDispute {
                id: other_client_id,
                amount: dec!(12.01),
                available: dec!(12)
            }),
            "should fail to move more funds than available"
        );
        assert_eq!(
            clients.database[&other_client_id].available_amount,
            dec!(12),
            "should not update available amount"
        );
        assert_eq!(
            clients.database[&other_client_id].held_amount,
            dec!(31),
            "should not update held amount"
        );

        // Should fail to move funds on a locked account
        assert_eq!(
            clients.move_to_held(locked_client_id, dec!(1)),
            Err(Error::ClientLocked(locked_client_id)),
            "should fail to move funds on a locked account"
        );
        assert_eq!(
            clients.database[&locked_client_id].available_amount,
            dec!(100),
            "should not update available amount"
        );
        assert_eq!(
            clients.database[&locked_client_id].held_amount,
            dec!(11),
            "should not update held amount"
        );

        Ok(())
    }

    #[test]
    fn test_move_to_available() -> Result<(), Error> {
        let mut clients = Clients::default();

        let client_id = 94;
        let other_client_id = 95;
        let locked_client_id = 96;
        clients.database.insert(
            client_id,
            Client {
                available_amount: dec!(95.6),
                held_amount: dec!(131.33),
                locked: false,
            },
        );
        clients.database.insert(
            other_client_id,
            Client {
                available_amount: dec!(12),
                held_amount: dec!(31),
                locked: false,
            },
        );
        clients.database.insert(
            locked_client_id,
            Client {
                available_amount: dec!(100),
                held_amount: dec!(11),
                locked: true,
            },
        );

        // Should correctly move funds from available to held
        clients.move_to_available(client_id, dec!(30))?;
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(125.6),
            "should correctly increase available funds"
        );
        assert_eq!(
            clients.database[&client_id].held_amount,
            dec!(101.33),
            "should correctly decrease held funds"
        );

        // Should fail to move more funds than held
        assert_eq!(
            clients.move_to_available(other_client_id, dec!(31.01)),
            Err(Error::ClientCannotResolve {
                id: other_client_id,
                amount: dec!(31.01),
                held: dec!(31)
            }),
            "should fail to move more funds than in held"
        );
        assert_eq!(
            clients.database[&other_client_id].available_amount,
            dec!(12),
            "should not update available amount"
        );
        assert_eq!(
            clients.database[&other_client_id].held_amount,
            dec!(31),
            "should not update held amount"
        );

        // Should fail to move funds on a locked account
        assert_eq!(
            clients.move_to_available(locked_client_id, dec!(1)),
            Err(Error::ClientLocked(locked_client_id)),
            "should fail to move funds on a locked account"
        );
        assert_eq!(
            clients.database[&locked_client_id].available_amount,
            dec!(100),
            "should not update available amount"
        );
        assert_eq!(
            clients.database[&locked_client_id].held_amount,
            dec!(11),
            "should not update held amount"
        );

        Ok(())
    }

    #[test]
    fn test_chargeback() -> Result<(), Error> {
        let mut clients = Clients::default();

        let client_id = 94;
        let other_client_id = 95;
        let locked_client_id = 96;
        clients.database.insert(
            client_id,
            Client {
                available_amount: dec!(95.6),
                held_amount: dec!(131.33),
                locked: false,
            },
        );
        clients.database.insert(
            other_client_id,
            Client {
                available_amount: dec!(12),
                held_amount: dec!(31),
                locked: false,
            },
        );
        clients.database.insert(
            locked_client_id,
            Client {
                available_amount: dec!(100),
                held_amount: dec!(11),
                locked: true,
            },
        );

        // Should chargeback and lock account
        clients.chargeback(client_id, dec!(100))?;
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(95.6),
            "should not update available ammount"
        );
        assert_eq!(
            clients.database[&client_id].held_amount,
            dec!(31.33),
            "should properly update held amount"
        );
        assert!(
            clients.database[&client_id].locked,
            "client should now be locked"
        );

        // Should fail to chargeback if amount is higher than held
        assert_eq!(
            clients.chargeback(other_client_id, dec!(31.01)),
            Err(Error::ClientCannotChargeBack {
                id: other_client_id,
                amount: dec!(31.01),
                held: dec!(31)
            }),
            "should fail chargeback if not enough money in held"
        );
        assert_eq!(
            clients.database[&other_client_id].available_amount,
            dec!(12),
            "should not update available amount"
        );
        assert_eq!(
            clients.database[&other_client_id].held_amount,
            dec!(31),
            "should not update held amount"
        );
        assert!(
            clients.database[&other_client_id].locked,
            "client should now be locked"
        );

        // Should fail to chargeback an already locked account
        assert_eq!(
            clients.chargeback(locked_client_id, dec!(1)),
            Err(Error::ClientLocked(locked_client_id)),
            "should fail to chargeback an already locked account"
        );
        assert_eq!(
            clients.database[&locked_client_id].available_amount,
            dec!(100),
            "should not update available amount"
        );
        assert_eq!(
            clients.database[&locked_client_id].held_amount,
            dec!(11),
            "should not update held amount"
        );
        assert!(
            clients.database[&locked_client_id].locked,
            "client should still be locked"
        );

        Ok(())
    }
}

use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};
use rust_decimal::Decimal;

use crate::dtos::ClientID;

// DAO (representation of what would be our Clients table in the database)
#[derive(Default)]
pub struct Client {
    pub available_amount: Decimal,
    pub held_amount: Decimal,
    pub locked: bool,
}

// In a proper implementation, the Clients store would connect to a database instead of being an in-memory store
#[derive(Default)]
pub struct Clients {
    database: HashMap<ClientID, Client>,
}

impl Clients {
    pub fn deposit(&mut self, id: ClientID, amount: Decimal) -> Result<()> {
        let client = self.database.entry(id).or_insert_with(Client::default);
        if client.locked {
            bail!("Client {} is locked", id);
        }

        client.available_amount += amount;

        Ok(())
    }

    pub fn withdrawal(&mut self, id: ClientID, amount: Decimal) -> Result<()> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or_else(|| anyhow!("Client {} does not exist", id))?;
        if client.locked {
            bail!("Client {} is locked", id);
        }

        let new_amount = client.available_amount - amount;
        if new_amount.is_sign_negative() {
            bail!(
                "Client {} cannot withdrawl {} as available balance is {}",
                id,
                amount,
                client.available_amount
            );
        }

        client.available_amount = new_amount;

        Ok(())
    }

    pub fn move_to_held(&mut self, id: ClientID, amount: Decimal) -> Result<()> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or_else(|| anyhow!("Client {} does not exist", id))?;
        if client.locked {
            bail!("Client {} is locked", id);
        }

        let new_available = client.available_amount - amount;
        if new_available.is_sign_negative() {
            bail!(
                "Client {} cannot hold {} as available balance is {}",
                id,
                amount,
                client.available_amount
            );
        }

        client.available_amount = new_available;
        client.held_amount += amount;

        Ok(())
    }

    pub fn move_to_available(&mut self, id: ClientID, amount: Decimal) -> Result<()> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or_else(|| anyhow!("Client {} does not exist", id))?;
        if client.locked {
            bail!("Client {} is locked", id);
        }

        let new_held = client.held_amount - amount;
        if new_held.is_sign_negative() {
            bail!(
                "Client {} cannot free {} as held balance is {}",
                id,
                amount,
                client.held_amount
            );
        }

        client.held_amount = new_held;
        client.available_amount += amount;

        Ok(())
    }

    pub fn chargeback(&mut self, id: ClientID, amount: Decimal) -> Result<()> {
        let client = self
            .database
            .get_mut(&id)
            .ok_or_else(|| anyhow!("Client {} does not exist", id))?;
        if client.locked {
            bail!("Client {} is locked", id);
        }

        client.locked = true;

        let new_held = client.held_amount - amount;
        if new_held.is_sign_negative() {
            bail!(
                "Client {} cannot chargeback {} as held balance is {}",
                id,
                amount,
                client.held_amount
            );
        }

        client.held_amount = new_held;
        Ok(())
    }

    pub fn get_client(&self, id: ClientID) -> Result<&Client> {
        self.database
            .get(&id)
            .ok_or_else(|| anyhow!("Client {} does not exist", id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::Result;
    use rust_decimal_macros::dec;

    #[test]
    fn test_deposit() -> Result<()> {
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
        assert!(
            clients.deposit(other_client_id, dec!(1)).is_err(),
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
    fn test_withdrawal() -> Result<()> {
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
        assert!(
            clients.withdrawal(other_client_id, dec!(1)).is_err(),
            "should fail to withdrawl higher than available amount"
        );
        assert_eq!(
            clients.database[&client_id].available_amount,
            dec!(95),
            "available amount should be unmodified"
        );

        // Cannot withdrawl from locked account
        assert!(
            clients.withdrawal(locked_client_id, dec!(1)).is_err(),
            "should fail to withdrawl from locked account"
        );
        assert_eq!(
            clients.database[&locked_client_id].available_amount,
            dec!(100),
            "available amount should be unmodified"
        );

        // Cannot withdrawl from non-existant account
        assert!(
            clients.withdrawal(non_exist_client_id, dec!(1)).is_err(),
            "should fail to withdrawl from non existant account"
        );

        Ok(())
    }

    #[test]
    fn test_move_to_held() -> Result<()> {
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
        assert!(
            clients.move_to_held(other_client_id, dec!(12.01)).is_err(),
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
        assert!(
            clients.move_to_held(locked_client_id, dec!(1)).is_err(),
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
    fn test_move_to_available() -> Result<()> {
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
        assert!(
            clients
                .move_to_available(other_client_id, dec!(31.01))
                .is_err(),
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
        assert!(
            clients
                .move_to_available(locked_client_id, dec!(1))
                .is_err(),
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
    fn test_chargeback() -> Result<()> {
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
        assert!(
            clients.chargeback(other_client_id, dec!(31.01)).is_err(),
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
        assert!(
            clients.chargeback(locked_client_id, dec!(1)).is_err(),
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

    #[test]
    fn test_get_client() -> Result<()> {
        let mut clients = Clients::default();

        let client_id = 12;
        let missing_client_id = 13;

        clients.database.insert(
            client_id,
            Client {
                available_amount: dec!(1.1),
                held_amount: dec!(1.1),
                locked: false,
            },
        );

        let client = clients.get_client(client_id)?;
        assert_eq!(
            client.available_amount,
            dec!(1.1),
            "client available amount should match"
        );

        assert!(
            clients.get_client(missing_client_id).is_err(),
            "client with invalid id should fail"
        );

        Ok(())
    }
}

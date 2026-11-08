//! Durable transfer outbox and import receipts share the inventory's atomic save.
use super::*;
use crate::survivors::shared_chest::Transfer;

/// Cached per build update; chest snapshots never reload item configuration.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default, Deserialize)]
pub(in crate::survivors) struct TradeAccount {
    #[serde(rename = "trade_identity", default)]
    pub identity: String,
    #[serde(default)]
    pub pending_deposits: Vec<Transfer>,
}
#[cfg(not(target_arch = "wasm32"))]
impl TradeAccount {
    pub fn decode(text: &str) -> Self {
        toml::from_str(text).unwrap_or_default()
    }
}

impl Inventory {
    pub(in crate::survivors) fn trade_identity(&self) -> &str {
        &self.save.trade_identity
    }
    pub(in crate::survivors) fn pending_deposits(&self) -> &[Transfer] {
        &self.save.pending_deposits
    }
    pub(in crate::survivors) fn ensure_trade_identity(&mut self) -> Result<String, String> {
        if self.save.trade_identity.is_empty() {
            let mut save = self.save.clone();
            save.trade_identity = crate::survivors::shared_chest::nonce();
            self.commit(save)?;
        }
        Ok(self.save.trade_identity.clone())
    }
    pub(in crate::survivors) fn begin_deposit(
        &mut self,
        id: u64,
        vault: &str,
    ) -> Result<(), String> {
        if vault.is_empty() {
            return Err("The chest is not available yet.".into());
        }
        if self.worn(id) {
            return Err("Unequip this item before depositing it.".into());
        }
        let item = self.item(id).cloned().ok_or("Item not found.")?;
        let mut save = self.save.clone();
        if save.trade_identity.is_empty() {
            save.trade_identity = crate::survivors::shared_chest::nonce();
        }
        save.trade_sequence = save
            .trade_sequence
            .checked_add(1)
            .ok_or("No more transfer identifiers available.")?;
        let token = format!("{}:{}", save.trade_identity, save.trade_sequence);
        save.items.retain(|i| i.id != id);
        save.pending_deposits.push(Transfer {
            token,
            vault: vault.into(),
            item,
        });
        self.commit(save)
    }
    pub(in crate::survivors) fn finish_deposit(&mut self, token: &str) -> Result<(), String> {
        if !self.save.pending_deposits.iter().any(|t| t.token == token) {
            return Ok(());
        }
        let mut save = self.save.clone();
        save.pending_deposits.retain(|t| t.token != token);
        self.commit(save)
    }
    pub(in crate::survivors) fn receive_trade(
        &mut self,
        transfer: &Transfer,
    ) -> Result<(), String> {
        if self.save.received_trades.contains(&transfer.token) {
            return Ok(());
        }
        let mut save = self.save.clone();
        let mut item = transfer.item.clone();
        item.id = save.next_id;
        save.next_id = save
            .next_id
            .checked_add(1)
            .ok_or("No more item identifiers available.")?;
        save.items.push(item);
        save.received_trades.insert(transfer.token.clone());
        self.commit(save)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_saves_leave_deposits_and_import_receipts_unchanged() {
        let item = crate::survivors::shared_chest::fixture_item();
        let mut inventory = Inventory::fixture(vec![item.clone()]);
        inventory.blocked = true;
        assert!(inventory.begin_deposit(1, "vault").is_err());
        assert!(inventory.item(1).is_some());
        assert!(inventory.pending_deposits().is_empty());
        let transfer = Transfer {
            token: "unique-token".into(),
            vault: "vault".into(),
            item,
        };
        assert!(inventory.receive_trade(&transfer).is_err());
        assert!(!inventory.save.received_trades.contains("unique-token"));
        inventory.blocked = false;
        inventory.receive_trade(&transfer).unwrap();
        inventory.receive_trade(&transfer).unwrap();
        assert_eq!(inventory.all().len(), 2);
    }
}

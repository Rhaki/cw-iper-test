use cosmwasm_schema::cw_serde;
use cosmwasm_std::Storage;
use cw_storage_plus::Item;

use crate::error::AppResult;

#[cw_serde]
/// Structure containing the basic info of a [`IperApp`](crate::IperApp).
///
/// When creating a [`IperApp`](crate::IperApp), this structure is saved in `Storage`. It is possible to load it from any point of an [`IbcApplication`] / `StartgateApplication` via the `load` method
pub struct ChainHelper {
    /// `chain_prefix` of the chain
    pub chain_prefix: String,
}

impl ChainHelper {
    const KEY: &'static str = "chain_helper_key";
}

impl ChainHelper {
    /// Load the stored `ChainHelper`
    pub fn load(storage: &dyn Storage) -> AppResult<Self> {
        Ok(Item::new(Self::KEY).load(storage)?)
    }

    /// Save the current data into the `Storage`
    pub fn save(&self, storage: &mut dyn Storage) -> AppResult<()> {
        Ok(Item::new(Self::KEY).save(storage, self)?)
    }
}

use cosmwasm_schema::cw_serde;
use cosmwasm_std::Storage;
use cw_storage_plus::Item;

use crate::error::AppResult;

#[cw_serde]
pub struct ChainHelper {
    pub chain_prefix: String,
}

impl ChainHelper {
    const KEY: &'static str = "chain_helper_key";
}

impl ChainHelper {
    pub fn load(storage: &dyn Storage) -> AppResult<Self> {
        Ok(Item::new(Self::KEY).load(storage)?)
    }

    pub fn save(&self, storage: &mut dyn Storage) -> AppResult<()> {
        Ok(Item::new(Self::KEY).save(storage, self)?)
    }
}

use crate::types::Result;
use openbrush::{
    modifiers,
    traits::AccountId,
};

#[openbrush::wrapper]
pub type LazyMintProxyRef = dyn LazyMintProxy;

#[openbrush::trait_definition]
pub trait LazyMintProxy {
    #[ink(message)]
    fn rmrk_contract_address(&self) -> AccountId;

    #[ink(message)]
    fn catalog_contract_address(&self) -> AccountId;

    #[ink(message)]
    #[modifiers(only_owner)]
    fn set_rmrk_contract_address(&mut self, new_contract_address: AccountId) -> Result<()>;

    #[ink(message)]
    #[modifiers(only_owner)]
    fn set_catalog_contract_address(&mut self, new_contract_address: AccountId) -> Result<()>;
}

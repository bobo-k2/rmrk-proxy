use openbrush::{
    contracts::ownable::OwnableError,
    traits::AccountId,
};

pub const STORAGE_KEY: u32 = openbrush::storage_unique_key!(Data);

#[derive(Default, Debug)]
#[openbrush::upgradeable_storage(STORAGE_KEY)]
pub struct Data {
    pub rmrk_contract: Option<AccountId>,
    pub catalog_contract: Option<AccountId>,
    pub salt: u64, // used for pseudo random number generation
}

#[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ProxyError {
    /// Caller is not a marketplace owner.
    OwnableError(OwnableError),
    MintingError,
    OwnershipTransferError,
    AddTokenAssetError,
    NoAssetsDefined,
    TooManyAssetsDefined,
}

pub type Result<T> = core::result::Result<T, ProxyError>;

impl From<OwnableError> for ProxyError {
    fn from(error: OwnableError) -> Self {
        ProxyError::OwnableError(error)
    }
}
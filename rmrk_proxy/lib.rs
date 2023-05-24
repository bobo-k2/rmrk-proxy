#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod rmrk_proxy {
    use crate::ensure;
    use ink::{
        env::{
            call::{
                build_call,
                ExecutionInput,
                Selector,
            },
            hash,
            DefaultEnvironment,
        },
        prelude::vec::Vec,
    };
    use openbrush::contracts::psp34::Id;

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        MintingError,
        OwnershipTransferError,
        NoAssetsDefined,
        TooManyAssets,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct RmrkProxy {
        rmrk_contract: AccountId,
        catalog_contract: AccountId,
        salt: u64,
    }

    impl RmrkProxy {
        #[ink(constructor)]
        pub fn new(rmrk_contract: AccountId, catalog_contract: AccountId) -> Self {
            // TODO check if it is possible to get minting price from rmrk contract. If no add it as a parameter.
            Self {
                rmrk_contract,
                catalog_contract,
                salt: 0,
            }
        }

        #[ink(message, payable)]
        pub fn mint(&mut self) -> Result<()> {
            let transferred_value = Self::env().transferred_value();
            let caller = Self::env().caller();

            let total_assets = build_call::<DefaultEnvironment>()
                .call(self.rmrk_contract)
                .gas_limit(5000000000)
                .exec_input(ExecutionInput::new(Selector::new(ink::selector_bytes!(
                    "MultiAsset::total_assets"
                ))))
                .returns::<u32>()
                .try_invoke()
                .unwrap();
            ensure!(total_assets.unwrap() > 0, Error::NoAssetsDefined);
            // This is temporary since current pseudo random generator is not working with big numbers.
            ensure!(total_assets.unwrap() < 256, Error::TooManyAssets);

            // TODO check why the call is failing silently when no transferred value is provided.
            let _mint_result = build_call::<DefaultEnvironment>()
                .call(self.rmrk_contract)
                .gas_limit(5000000000)
                .transferred_value(transferred_value)
                .exec_input(ExecutionInput::new(Selector::new(ink::selector_bytes!(
                    "MintingLazy::mint"
                ))))
                .returns::<()>()
                .try_invoke()
                .map_err(|_| Error::MintingError)?;

            let asset_id = self.get_pseudo_random((total_assets.unwrap() -1)  as u8);
            let _add_asset_result = build_call::<DefaultEnvironment>()
                .call(self.rmrk_contract)
                .gas_limit(5000000000)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!(
                        "MultiAsset::add_asset_to_token"
                    )))
                    .push_arg(Id::U64(1))
                    .push_arg(asset_id as u32),
                )
                .returns::<()>()
                .try_invoke();

            let _transfer_token_result = build_call::<DefaultEnvironment>()
                .call(self.rmrk_contract)
                .gas_limit(5000000000)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("PSP34::transfer")))
                        .push_arg(caller)
                        .push_arg(Id::U64(1))
                        .push_arg(Vec::<u8>::new()),
                )
                .returns::<()>()
                .try_invoke()
                .map_err(|_| Error::OwnershipTransferError)?;

            Ok(())
        }

        #[ink(message)]
        pub fn get_rmrk_contract_address(&self) -> AccountId {
            self.rmrk_contract
        }

        #[ink(message)]
        pub fn get_catalog_contract_address(&self) -> AccountId {
            self.catalog_contract
        }

        /// Generates pseudo random number, Used to pick a random asset for a token. Big assumption is that we will have less than 256 assets.
        fn get_pseudo_random(&mut self, max_value: u8) -> u8 {
            let seed = self.env().block_timestamp();
            let mut input: Vec<u8> = Vec::new();
            input.extend_from_slice(&seed.to_be_bytes());
            input.extend_from_slice(&self.salt.to_be_bytes());
            let mut output = <hash::Keccak256 as hash::HashOutput>::Type::default();
            ink::env::hash_bytes::<hash::Keccak256>(&input, &mut output);
            self.salt += 1;
            let number = output[0] % (max_value + 1);
            number
        }
    }

    /// Unit tests in Rust are normally defined within such a `#[cfg(test)]`
    /// module and test functions are marked with a `#[test]` attribute.
    /// The below code is technically just normal Rust code.
    #[cfg(test)]
    mod tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;

        /// We test a simple use case of our contract.
        #[ink::test]
        fn constructor_works() {
            let rmrk: AccountId = [0x42; 32].into();
            let catalog: AccountId = [0x41; 32].into();

            let contract = RmrkProxy::new(rmrk, catalog);
            assert_eq!(contract.get_rmrk_contract_address(), rmrk);
            assert_eq!(contract.get_catalog_contract_address(), catalog);
        }
    }

    // /// This is how you'd write end-to-end (E2E) or integration tests for ink! contracts.
    // ///
    // /// When running these you need to make sure that you:
    // /// - Compile the tests with the `e2e-tests` feature flag enabled (`--features e2e-tests`)
    // /// - Are running a Substrate node which contains `pallet-contracts` in the background
    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e_tests {
        /// Imports all the definitions from the outer scope so we can use them here.
        use super::*;
        use crate::rmrk_proxy::RmrkProxyRef;
        use catalog_example::catalog_example::CatalogContractRef;
        use ink::primitives::AccountId;
        use ink_e2e::build_message;
        use openbrush::contracts::psp34::{
            psp34_external::PSP34,
            Id,
        };
        use rmrk::{
            storage::catalog_external::Catalog,
            traits::multiasset_external::MultiAsset,
            types::{
                Part,
                PartType,
            },
        };
        use rmrk_equippable_lazy::rmrk_equippable_lazy::RmrkRef;

        type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

        #[ink_e2e::test]
        async fn mint_works(mut client: ink_e2e::Client<C, E>) -> E2EResult<()> {
            let alice = ink_e2e::alice();

            let catalog_constructor = CatalogContractRef::new(String::from("ipfs://").into());
            let catalog_contract_address = client
                .instantiate("catalog_example", &alice, catalog_constructor, 0, None)
                .await
                .expect("Catalog contract instantiation failed")
                .account_id;

            // Add part to catalog
            let part_ids = vec![1];
            let parts = vec![Part {
                part_type: PartType::Fixed,
                z: 0,
                equippable: vec![],
                part_uri: String::from("ipfs://").into(),
                is_equippable_by_all: false,
            }];
            let add_part_message =
                build_message::<CatalogContractRef>(catalog_contract_address.clone())
                    .call(|catalog| catalog.add_part_list(part_ids.clone(), parts.clone()));
            client
                .call(&alice, add_part_message, 0, None)
                .await
                .expect("Add part failed");
            let read_parts_count_message =
                build_message::<CatalogContractRef>(catalog_contract_address.clone())
                    .call(|catalog| catalog.get_parts_count());
            let read_parts_count_result = client
                .call_dry_run(&alice, &read_parts_count_message, 0, None)
                .await
                .return_value();
            assert_eq!(read_parts_count_result, 1);

            // RMRK contract
            let rmrk_constructor = RmrkRef::new(
                String::from("Test").into(),
                String::from("TST").into(),
                String::from("ipfs://base").into(),
                None,
                1_000_000_000_000_000_000,
                String::from("ipfs://collection").into(),
                AccountId::try_from(alice.account_id().as_ref()).unwrap(),
                1,
            );
            let rmrk_address = client
                .instantiate("rmrk_equippable_lazy", &alice, rmrk_constructor, 0, None)
                .await
                .expect("RMRK contract instantiation failed")
                .account_id;

            // Add asset to RMRK contract.
            let add_asset_entry_message =
                build_message::<RmrkRef>(rmrk_address.clone()).call(|rmrk| {
                    rmrk.add_asset_entry(
                        Some(catalog_contract_address.clone()),
                        0,
                        1,
                        String::from("ipfs://parturi").into(),
                        vec![0],
                    )
                });
            client
                .call(&alice, add_asset_entry_message, 0, None)
                .await
                .expect("Add asset entry failed");

            // Proxy contract
            let proxy_constructor = RmrkProxyRef::new(rmrk_address, catalog_contract_address);
            let proxy_address = client
                .instantiate("rmrk_proxy", &alice, proxy_constructor, 0, None)
                .await
                .expect("Proxy contract instantiation failed")
                .account_id;

            // Mint
            let mint_message =
                build_message::<RmrkProxyRef>(proxy_address.clone()).call(|proxy| proxy.mint());
            client
                .call(&alice, mint_message, 1_000_000_000_000_000_000, None)
                .await
                .expect("Mint failed");

            // Check if token was minted
            let read_total_supply_message =
                build_message::<RmrkRef>(rmrk_address.clone()).call(|rmrk| rmrk.total_supply());

            let read_total_supply_result = client
                .call_dry_run(&alice, &read_total_supply_message, 0, None)
                .await
                .return_value();
            assert_eq!(read_total_supply_result, 1);

            // Check if asset has been added to token
            let read_total_assets_message =
                build_message::<RmrkRef>(rmrk_address.clone()).call(|rmrk| rmrk.total_assets());

            let read_total_assets_result = client
                .call_dry_run(&alice, &read_total_assets_message, 0, None)
                .await
                .return_value();
            assert_eq!(read_total_assets_result, 1);

            // Check if token owner is correct
            let read_owner_of_message = build_message::<RmrkRef>(rmrk_address.clone())
                .call(|rmrk| rmrk.owner_of(Id::U64(1)));

            let read_owner_of_result = client
                .call_dry_run(&alice, &read_owner_of_message, 0, None)
                .await
                .return_value()
                .unwrap();
            ink::env::debug_println!(
                "token_owner: {:?} {:?}",
                read_owner_of_result,
                *alice.account_id()
            );
            // assert_eq!(read_owner_of_result, *alice.account_id().);

            Ok(())
        }
    }
}

/// Evaluate `$x:expr` and if not true return `Err($y:expr)`.
///
/// Used as `ensure!(expression_to_ensure, expression_to_return_on_false)`.
#[macro_export]
macro_rules! ensure {
    ( $x:expr, $y:expr $(,)? ) => {{
        if !$x {
            return Err($y.into())
        }
    }};
}

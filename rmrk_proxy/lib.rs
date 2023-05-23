#![cfg_attr(not(feature = "std"), no_std)]

#[ink::contract]
mod rmrk_proxy {
    use ink::env::{
        call::{
            build_call,
            ExecutionInput,
            Selector,
        },
        DefaultEnvironment,
    };

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        MintingError,
    }

    pub type Result<T> = core::result::Result<T, Error>;

    #[ink(storage)]
    pub struct RmrkProxy {
        rmrk_contract: AccountId,
        catalog_contract: AccountId,
    }

    impl RmrkProxy {
        #[ink(constructor)]
        pub fn new(rmrk_contract: AccountId, catalog_contract: AccountId) -> Self {
            // TODO check if it is possible to get minting price from rmrk contract. If no add it as a parameter.
            Self {
                rmrk_contract,
                catalog_contract,
            }
        }

        #[ink(message, payable)]
        pub fn mint(&mut self) -> Result<()> {
            // let transferred_value = Self::env().transferred_value();
            let caller = Self::env().caller();

            let _mint_result = build_call::<DefaultEnvironment>()
                .call(self.rmrk_contract)
                .gas_limit(5000000000)
                .exec_input(ExecutionInput::new(Selector::new(ink::selector_bytes!(
                    "Rmrk::mint"
                ))))
                .returns::<()>()
                .try_invoke();
            ink::env::debug_println!("mint_result: {:?}", _mint_result);

            let _add_asset_result = build_call::<DefaultEnvironment>()
                .call(self.rmrk_contract)
                .gas_limit(5000000000)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!(
                        "Rmrk::add_asset_to_token"
                    )))
                    .push_arg(1)
                    .push_arg(0),
                )
                .returns::<()>()
                .try_invoke();

            let _transfer_token_result = build_call::<DefaultEnvironment>()
                .call(self.rmrk_contract)
                .gas_limit(5000000000)
                .exec_input(
                    ExecutionInput::new(Selector::new(ink::selector_bytes!("Rmrk::transfer_to")))
                        .push_arg(caller)
                        .push_arg(1),
                )
                .returns::<()>()
                .try_invoke();

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

            let cotract = RmrkProxy::new(rmrk, catalog);
            assert_eq!(cotract.get_rmrk_contract_address(), rmrk);
            assert_eq!(cotract.get_catalog_contract_address(), catalog);
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
        use ink_e2e::build_message;
        use crate::rmrk_proxy::RmrkProxyRef;
        use catalog_example::catalog_example::CatalogContractRef;
        use rmrk_equippable_lazy::rmrk_equippable_lazy::RmrkRef;
        use rmrk::{
            storage::catalog_external::Catalog,
            types::{
                Part,
                PartType,
            },
        };
        use openbrush::contracts::psp34::psp34_external::PSP34;
        
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
            // ink::env::debug_println!("rmrk_address: {:?}", rmrk_address);

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
                .call(&ink_e2e::alice(), mint_message, 0, None)
                .await
                .expect("Mint failed");

            // Check if token was minted
            let read_total_supply_message =
                build_message::<RmrkRef>(rmrk_address.clone()).call(|rmrk| rmrk.total_supply());

            let read_total_supply_result = client
                .call_dry_run(&ink_e2e::alice(), &read_total_supply_message, 0, None)
                .await
                .return_value();
            assert_eq!(read_total_supply_result, 1);

            Ok(())
        }
    }
}

use multiversx_sc::{codec::multi_types::OptionalValue, types::Address};
use multiversx_sc_scenario::{managed_address, managed_biguint, rust_biguint, DebugApi};
use multiversx_sc_scenario::imports::{BlockchainStateWrapper, ContractObjWrapper};
use staking_smart::*;

// Constantlar
const WASM_PATH: &'static str = "output/staking-smart.wasm";
const USER_BALANCE: u64 = 1_000_000_000_000_000_000;
const APY: u64 = 1_000; // 10%
const BLOCKS_IN_YEAR: u64 = 60 * 60 * 24 * 365 / 6; // Assume 1 year in blocks
const MAX_PERCENTAGE: u64 = 10_000; // Max percentage (100%)

// Setup Contract
struct ContractSetup<ContractObjBuilder>
where
    ContractObjBuilder: 'static + Copy + Fn() -> staking_smart::ContractObj<DebugApi>,
{
    pub b_mock: BlockchainStateWrapper,
    pub owner_address: Address,
    pub user_address: Address,
    pub contract_wrapper:
        ContractObjWrapper<staking_smart::ContractObj<DebugApi>, ContractObjBuilder>,
}

impl<ContractObjBuilder> ContractSetup<ContractObjBuilder>
where
    ContractObjBuilder: 'static + Copy + Fn() -> staking_smart::ContractObj<DebugApi>,
{
    pub fn new(sc_builder: ContractObjBuilder) -> Self {
        let rust_zero = rust_biguint!(0u64);
        let mut b_mock = BlockchainStateWrapper::new();
        let owner_address = b_mock.create_user_account(&rust_zero);
        let user_address = b_mock.create_user_account(&rust_biguint!(USER_BALANCE));
        let sc_wrapper =
            b_mock.create_sc_account(&rust_zero, Some(&owner_address), sc_builder, WASM_PATH);

        // Simulate deploy
        b_mock
            .execute_tx(&owner_address, &sc_wrapper, &rust_zero, |sc| {
                sc.init(APY);
            })
            .assert_ok();

        ContractSetup {
            b_mock,
            owner_address,
            user_address,
            contract_wrapper: sc_wrapper,
        }
    }
}

#[test]
fn rewards_test() {
    let mut setup = ContractSetup::new(staking_smart::contract_obj);
    let user_addr = setup.user_address.clone();

    // Stake full amount
    setup
        .b_mock
        .execute_tx(
            &user_addr,
            &setup.contract_wrapper,
            &rust_biguint!(USER_BALANCE),
            |sc| {
                sc.stake();

                // Assert that the stake amount is correctly set
                let staking_position = sc.staking_position(&managed_address!(&user_addr)).get();
                assert_eq!(
                    staking_position.stake_amount,
                    managed_biguint!(USER_BALANCE)
                );
                assert_eq!(staking_position.last_action_block, 0);
            },
        )
        .assert_ok();

    // Simulate passage of time (advance blocks)
    setup.b_mock.set_block_nonce(BLOCKS_IN_YEAR);

    // Query rewards before claiming
    setup
        .b_mock
        .execute_query(&setup.contract_wrapper, |sc| {
            let actual_rewards = sc.calculate_rewards_for_user(managed_address!(&user_addr));
            let expected_rewards = managed_biguint!(USER_BALANCE) * APY / MAX_PERCENTAGE;
            assert_eq!(actual_rewards, expected_rewards);
        })
        .assert_ok();

    // Claim rewards
    setup
        .b_mock
        .execute_tx(
            &user_addr,
            &setup.contract_wrapper,
            &rust_biguint!(0), // No additional stake or unstake
            |sc| {
                // Ensure the staking position before claiming rewards
                let staking_position = sc.staking_position(&managed_address!(&user_addr)).get();
                assert_eq!(
                    staking_position.stake_amount,
                    managed_biguint!(USER_BALANCE)
                );
                assert_eq!(staking_position.last_action_block, 0);

                // Claim rewards
                sc.claim_rewards();

                // Assert that staking position is updated correctly
                let updated_staking_position = sc.staking_position(&managed_address!(&user_addr)).get();
                assert_eq!(
                    updated_staking_position.stake_amount,
                    managed_biguint!(USER_BALANCE)
                );
                assert_eq!(updated_staking_position.last_action_block, BLOCKS_IN_YEAR);
            },
        )
        .assert_ok();

    // Check user's EGLD balance after claiming rewards
    setup.b_mock.check_egld_balance(
        &user_addr,
        &(rust_biguint!(USER_BALANCE) * APY / MAX_PERCENTAGE),
    );

    // Query rewards after claiming (should be 0)
    setup
        .b_mock
        .execute_query(&setup.contract_wrapper, |sc| {
            let actual_rewards = sc.calculate_rewards_for_user(managed_address!(&user_addr));
            let expected_rewards = managed_biguint!(0);
            assert_eq!(actual_rewards, expected_rewards);
        })
        .assert_ok();
}

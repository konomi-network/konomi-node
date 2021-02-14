#![cfg_attr(not(feature = "std"), no_std)]
use codec::Codec;

sp_api::decl_runtime_apis! {
    pub trait LendingApi<AssetId, FixedU128, AccountId, Balance> where 
        AssetId: Codec,
        FixedU128: Codec,
        AccountId: Codec,
        Balance: Codec
    {
        fn supply_rate(id: AssetId) -> FixedU128;

        fn debt_rate(id: AssetId) -> FixedU128;

        // effective supply balance; borrow balance
        fn get_user_info(user: AccountId) -> (u64, u64, u64);
    }
}
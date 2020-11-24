#![cfg_attr(not(feature = "std"), no_std)]
use codec::Codec;

sp_api::decl_runtime_apis! {
    pub trait SwapApi<AssetId, Balance> where 
        AssetId: Codec,
        Balance: Codec 
    {
        fn calculate_output(in_id: AssetId, out_id: AssetId, amount_in: Balance) -> Balance;
    }
}
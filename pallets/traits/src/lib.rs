#![cfg_attr(not(feature = "std"), no_std)]

pub trait Oracle<AssetId, Rate> {
	fn get_rate(asset_id: AssetId) -> Rate;
}

pub trait MultiAsset<AccountId, AssetId, Balance> {
	fn transfer(from: AccountId, id: AssetId, to: AccountId, amount: Balance) -> sp_std::result::Result<(), &'static str>;
}
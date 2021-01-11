#![cfg_attr(not(feature = "std"), no_std)]

pub trait Oracle<AssetId, Balance> {
	fn get_rate(asset_id: AssetId) -> Balance;
}
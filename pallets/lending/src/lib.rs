#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_event, decl_module, decl_storage,
    StorageMap, StorageValue,
};
use sp_runtime::{DispatchResult as Result, RuntimeDebug};
use frame_system::{self as system, ensure_signed};
use sp_std::prelude::*;
use sp_std::{vec::Vec, convert::TryInto};
use pallet_assets as assets;
use codec::{Encode, Decode};
use substrate_fixed::{
    types::U64F64,
    traits::FromFixed
};

// TODO: fee, reserves
// TODO: loose couple
// TODO: child storage
// TODO: U64F64 as type

/// The module's configuration trait.
pub trait Trait: assets::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
}


/// Pending atomic swap operation.
#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct Pool<T: Trait> {
    pub enabled: bool,

    pub can_be_collateral: bool,

	pub asset: <T as assets::Trait>::AssetId,

	pub supply: T::Balance,

    pub debt: T::Balance,

    pub liquidation_threshold: U64F64,

    pub supply_threshold: U64F64,

    pub liquidation_bonus: U64F64,

    pub total_supply_index: U64F64,

    pub total_debt_index: U64F64,

    pub last_updated: T::BlockNumber // TODO: considering timestamp?
    
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct UserSupply<T: Trait> {
	/// Source of the swap.
	pub amount: <T as assets::Trait>::Balance,
	/// Action of this swap.
    pub index: u64,
    
    pub as_collateral: bool,
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct UserDebt<T: Trait> {
	/// Source of the swap.
	pub amount: <T as assets::Trait>::Balance,
	/// Action of this swap.
	pub index: u64,
}

decl_event!(
    pub enum Event<T>
    where <T as system::Trait>::AccountId,
    <T as assets::Trait>::Balance,
    <T as assets::Trait>::AssetId {
        /// Assets swap event
        AssetsSwapped(AccountId, AssetId, Balance, AssetId, Balance),

    }
);

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as Lending
    {
        pub UserDebts: double_map
        hasher(twox_64_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId
        => Option<UserDebt<T>>;

        pub UserSupplies: double_map
        hasher(twox_64_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId
        => Option<UserSupply<T>>;

        pub Pools get(fn pool): map hasher(twox_64_concat) T::AssetId => Option<Pool<T>>;

        pub UserCollaterals: map hasher(blake2_128_concat) T::AccountId => Vec<T::AssetId>;

        Tether get(fn test): u32;

    }

}

// The module's dispatchable functions.
decl_module! {
    pub struct Module<T: Trait> for enum Call where
        origin: T::Origin {
        // Initializing events
        // this is needed only if you are using events in your module
        fn deposit_event() = default;

        // end user related
 
        #[weight = 1]
        fn supply(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO check pool exists
            // 1 accrue interest
            Self::accrue_interest(asset_id);
            // 2 transfer asset
            // 3 update user supply
            // 4 update pool supply

            Ok(())
        }

        #[weight = 1]
        fn withdraw(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO check pool exists
            // 1 accrue interest
            Self::accrue_interest(asset_id);
            // 2 check collateral and pool cash = (deposit - borrow)
            // 3 update user supply
            // 4 transfer asset to user

            Ok(())
        }

        #[weight = 1]
        fn borrow(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO check pool exists
            // 1 accrue interest
            Self::accrue_interest(asset_id);
            // 2 check collateral
            // 3 update user Borrow
            // 4 update pool borrow
            // 5 transfer asset to user
            Ok(())
        }

        #[weight = 1]
        fn repay(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO check pool exists
            // 1 accrue interest
            Self::accrue_interest(asset_id);
            // 2 transfer token from user
            // 3 update user borrow: if all loan is repaid, clean up the loan
            // 4 update pool borrow

            Ok(())
        }

        #[weight = 1]
        fn choose_collateral(
            origin,
            asset_id: T::AssetId,
            as_collateral: bool
        ) -> Result {

            // if from true -> false, need to check collateral
            Ok(())
        }

        // arbitrager related

        #[weight = 1]
        fn liquidate(
            origin,
            target_user: T::AccountId,
            pay_asset_id: T::AssetId,
            get_asset_id: T::AssetId,
            pay_asset_amount: T::Balance
        ) -> Result {
            let account = ensure_signed(origin)?;

            // TODO check pool exists
            // 1 check if get_asset_id is enabled as collatoral
            // 2 accrue interest of pay and get asset
            Self::accrue_interest(pay_asset_id);
            Self::accrue_interest(get_asset_id);
            // 3 check if target user is under liquidation condition
            // 4 check if liquidation % is more than threshold 
            // 5 transfer token from arbitrager
            // 6 transfer collateral to arbitrager
            // 7 recalculate target user's borrow and supply in 2 pools

            Ok(())
        }
        
        // governance related

        #[weight = 1]
        fn init_pool(
            origin,
            id: T::AssetId,
            can_be_collateral: bool
        ) -> Result {

            Ok(())
        }
    }
}

impl<T: Trait> Module<T>
{
    fn accrue_interest(asset_id: T::AssetId) {
        // TODO avoid multi get pool
        // TODO use error of convert error
        // 1 get pool
        let mut pool = Self::pool(asset_id).unwrap();
        if pool.last_updated == <frame_system::Module<T>>::block_number() {
            return
        }
        // dealing with option

        // 3 get time span
        let interval_block_number = <frame_system::Module<T>>::block_number() - pool.last_updated;
		let elapsed_time_u32 = TryInto::try_into(interval_block_number)
			.ok()
			.expect("blockchain will not exceed 2^32 blocks; qed");
        let elapsed_time_U64F64 = U64F64::from_num(elapsed_time_u32);

        // 4  get rates and calculate interest
        let new_total_supply_index = Self::get_supply_rate(asset_id) * elapsed_time_U64F64 * pool.total_supply_index;
        let new_total_debt_index = Self::get_debt_rate(asset_id) * elapsed_time_U64F64 * pool.total_debt_index;

        // 5 update pool index, supply, debt, timestamp
        let supply = TryInto::<u128>::try_into(pool.supply)
            .ok()
            .expect("Balance is u128");
        let supply = supply * new_total_supply_index;
        let converted = u128::from_fixed(supply);
        pool.supply = TryInto::<T::Balance>::try_into(converted)
            .ok()
            .expect("Balance is u128");

        let debt = TryInto::<u128>::try_into(pool.debt)
            .ok()
            .expect("Balance is u128");
        let debt = debt * new_total_supply_index;
        let converted = u128::from_fixed(debt);
        pool.debt = TryInto::<T::Balance>::try_into(converted)
            .ok()
            .expect("Balance is u128");

        Pools::<T>::insert(asset_id, pool);

    }

    fn get_debt_rate(asset_id: T::AssetId) -> U64F64 {
        U64F64::from_num(1)
    }

    fn get_supply_rate(asset_id: T::AssetId) -> U64F64 {
        U64F64::from_num(1)
    }

    fn get_user_total_collaterals(account: T::AccountId) -> T::Balance {
        T::Balance::default()
    }

}

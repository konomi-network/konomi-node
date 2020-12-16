#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_event, decl_module, decl_storage, Parameter,
    StorageMap, StorageValue,
};
use sp_runtime::{DispatchResult as Result, RuntimeDebug};
use frame_system::{self as system, ensure_signed};
use sp_core::crypto::{UncheckedFrom, UncheckedInto};
use sp_std::prelude::*;
use sp_std::{marker::PhantomData, mem, vec::Vec};
use sp_runtime::traits::{
    Bounded, Hash, AtLeast32BitUnsigned, Zero,
};
use pallet_assets as assets;
use codec::{Encode, Decode};

/// The module's configuration trait.
pub trait Trait: assets::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;    /// 
    /// The global fee rate
    type FeeRate: Parameter + AtLeast32BitUnsigned + Default + Copy;
}


/// Pending atomic swap operation.
#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct Pool<T: Trait> {
    pub enabled: bool,

    pub can_be_collateral: bool,
	/// Source of the swap.
	pub asset: <T as assets::Trait>::AssetId,
	/// Action of this swap.
	pub supply: T::Balance,
	/// End block of the lock.
    pub debt: T::Balance,

    pub liquidation_threshold: T::Balance,

    pub supply_threshold: T::Balance,

    pub liquidation_bonus: T::Balance,

    pub total_supply_index: u64,

    pub total_debt_index: u64,
    
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
        hasher(blake2_128_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId
        => Option<UserDebt<T>>;

        pub UserSupplies: double_map
        hasher(blake2_128_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId
        => Option<UserSupply<T>>;

        pub Pools get(fn pool): map hasher(blake2_128_concat) T::AssetId => Option<Pool<T>>;

        pub UserCollaterals: map hasher(blake2_128_concat) T::AccountId => Vec<T::AssetId>;

        /// The global fee rate of this platform
        FeeRateGlobal get(fn fee_rate) config(): T::FeeRate;

    }

}

// The module's dispatchable functions.
decl_module! {
    pub struct Module<T: Trait> for enum Call where
        origin: T::Origin {
        // Initializing events
        // this is needed only if you are using events in your module
        fn deposit_event() = default;

        /// Set global fee rate, need root permission
        /// @origin
        /// @fee_rate    the global fee rate on each transaction
        #[weight = 1]
        pub fn set_fee_rate(origin, fee_rate: T::FeeRate) -> Result {
            //ensure_root(origin)?;
            <FeeRateGlobal<T>>::mutate(|fr| *fr = fee_rate);

            Ok(())
        }

        #[weight = 1]
        fn supply(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO

            Ok(())
        }

        #[weight = 1]
        fn withdraw(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO

            Ok(())
        }

        #[weight = 1]
        fn borrow(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO

            Ok(())
        }

        #[weight = 1]
        fn repay(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            let account = ensure_signed(origin)?;

            // TODO

            Ok(())
        }

        #[weight = 1]
        fn liquidate(
            origin,
            target_user: T::AccountId,
            pay_asset_id: T::AssetId,
            get_asset_id: T::AssetId,
            pay_asset_amount: T::Balance
        ) -> Result {
            let account = ensure_signed(origin)?;

            // TODO

            Ok(())
        }
        
    }
}

impl<T: Trait> Module<T>
{

    fn get_borrow_rate(asset_id: T::AssetId) -> T::Balance {
        T::Balance::default()
    }

    fn get_supply_rate(asset_id: T::AssetId) -> T::Balance {
        T::Balance::default()
    }

    fn get_user_total_collaterals(account: T::AccountId) -> T::Balance {
        T::Balance::default()
    }

}



/// helper function
fn u64_to_bytes(x: u64) -> [u8; 8] {
    unsafe { mem::transmute(x.to_le()) }
}

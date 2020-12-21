#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_event, decl_module, decl_storage, decl_error,
    StorageMap, StorageValue,
};
use sp_runtime::{
    DispatchResult as Result, RuntimeDebug,
    traits::{AccountIdConversion, Zero}, ModuleId
};
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
// TODO: add events
// TODO: U64F64 as type
// TODO: reduce pool storage read

const PALLET_ID: ModuleId = ModuleId(*b"Lending!");

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

    pub last_updated: T::BlockNumber, // TODO: considering timestamp?

    pub supply_apy: T::Balance, // tmp

    pub debt_apy: T::Balance // tmp
    
}

// tmp
#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct User<T: Trait> {
	/// Source of the swap.
	pub borrow_limit: T::Balance,
	/// Action of this swap.

    pub supply_balance: T::Balance,

    pub debt_balance: T::Balance,

}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct UserSupply<T: Trait> {
	/// Source of the swap.
	pub amount: <T as assets::Trait>::Balance,
	/// Action of this swap.
    pub index: U64F64,
    
    pub as_collateral: bool,
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct UserDebt<T: Trait> {
	/// Source of the swap.
	pub amount: <T as assets::Trait>::Balance,
	/// Action of this swap.
	pub index: U64F64,
}

decl_event!(
    pub enum Event<T>
    where <T as system::Trait>::AccountId,
    <T as assets::Trait>::Balance,
    <T as assets::Trait>::AssetId {

        Supplied(AssetId, AccountId, Balance),

        Withdrawn(AssetId, AccountId, Balance),

        Borrowed(AssetId, AccountId, Balance),

        Repaid(AssetId, AccountId, Balance),

    }
);

// This module's storage items.
decl_storage! {
    trait Store for Module<T: Trait> as Lending
    {
        pub UserDebts get(fn user_debt): double_map
        hasher(twox_64_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId
        => Option<UserDebt<T>>;

        pub UserSupplies get(fn user_supply): double_map
        hasher(twox_64_concat) T::AssetId, hasher(blake2_128_concat) T::AccountId
        => Option<UserSupply<T>>;

        pub Pools get(fn pool): map hasher(twox_64_concat) T::AssetId => Option<Pool<T>>;

        pub Users get(fn user): map hasher(blake2_128_concat) T::AccountId => Option<User<T>>;

        pub UserSupplySet get(fn user_supply_set): map hasher(blake2_128_concat) T::AccountId => Vec<T::AssetId>;
        pub UserDebtSet get(fn user_debt_set): map hasher(blake2_128_concat) T::AccountId => Vec<T::AssetId>;
    }

    add_extra_genesis {
        config(pools): Vec<(T::AssetId)>;

        build(|config: &GenesisConfig<T>| {
            for pool in config.pools.iter() {
                <Module<T>>::_init_pool(*pool, true);
            }
        })
    }

}

decl_error! {
	pub enum Error for Module<T: Trait> {
        TransferFailed,
        NotEnoughLiquidity,
        InsufficientCollateral,
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
            <assets::Module<T>>::transfer(
                account.clone(), // TODO: use reference
                asset_id,
                Self::account_id(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;

            // 3 update user supply
            Self::update_user_supply(asset_id, account.clone(), amount, true);
            // 4 update pool supply
            Self::update_pool_supply(asset_id, amount, true);

            Self::deposit_event(RawEvent::Supplied(asset_id, account.clone(), amount));

            let mut assets = Self::user_supply_set(account.clone());
            if !assets.iter().any(|x| *x == asset_id) {
                assets.push(asset_id);
                UserSupplySet::<T>::insert(account.clone(), assets);
            }

            Self::update_apys(asset_id);
            Self::_update_user(account);
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
            // 2 check collateral 
            
            
            // 3 check pool cash = (deposit - borrow) > amount
            let pool = Self::pool(asset_id).unwrap();
            if (pool.supply - pool.debt) < amount {
                Err(Error::<T>::NotEnoughLiquidity)?
            }

            // 4 transfer asset to user
            <assets::Module<T>>::transfer(
                Self::account_id(),
                asset_id,
                account.clone(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;

            // 5 update user supply
            Self::update_user_supply(asset_id, account.clone(), amount, false);

            // 6 update pool supply
            Self::update_pool_supply(asset_id, amount, false);

            Self::deposit_event(RawEvent::Withdrawn(asset_id, account.clone(), amount));

            Self::update_apys(asset_id);
            Self::_update_user(account);
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

            // 3 check pool cash = (deposit - borrow) > amount
            let pool = Self::pool(asset_id).unwrap();
            if (pool.supply - pool.debt) < amount {
                Err(Error::<T>::NotEnoughLiquidity)?
            }

            // 4 transfer asset to user
            <assets::Module<T>>::transfer(
                Self::account_id(),
                asset_id,
                account.clone(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;
            // 5 update user Borrow
            Self::update_user_debt(asset_id, account.clone(), amount, true);
            // 6 update pool borrow
            Self::update_pool_debt(asset_id, amount, true);

            Self::deposit_event(RawEvent::Borrowed(asset_id, account.clone(), amount));

            let mut assets = Self::user_debt_set(account.clone());
            if !assets.iter().any(|x| *x == asset_id) {
                assets.push(asset_id);
                UserDebtSet::<T>::insert(account.clone(), assets);
            }
            Self::update_apys(asset_id);
            Self::_update_user(account);
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
            <assets::Module<T>>::transfer(
                account.clone(), // TODO: use reference
                asset_id,
                Self::account_id(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;
            // 3 update user Borrow
            Self::update_user_debt(asset_id, account.clone(), amount, false);
            // 4 update pool borrow
            Self::update_pool_debt(asset_id, amount, false);

            Self::deposit_event(RawEvent::Repaid(asset_id, account.clone(), amount));

            Self::update_apys(asset_id);
            Self::_update_user(account);
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

            Self::_init_pool(id, can_be_collateral);

            Ok(())
        }
    }
}

impl<T: Trait> Module<T>
{

    fn account_id() -> T::AccountId {
		PALLET_ID.into_account()
    }
    
    fn accrue_interest(asset_id: T::AssetId) {
        // TODO avoid multi get pool
        // TODO use error of convert error
        // TODO use compound interest
        // 1 get pool
        let mut pool = Self::pool(asset_id).unwrap();
        if pool.last_updated == <frame_system::Module<T>>::block_number() {
            return
        }

        // 3 get time span
        let interval_block_number = <frame_system::Module<T>>::block_number() - pool.last_updated;
		let elapsed_time_u32 = TryInto::try_into(interval_block_number)
			.ok()
			.expect("blockchain will not exceed 2^32 blocks; qed");
        let elapsed_time_U64F64 = U64F64::from_num(elapsed_time_u32);

        // 4  get rates and calculate interest
        let supply_multiplier = U64F64::from_num(1) + Self::get_supply_rate(asset_id) * elapsed_time_U64F64;
        let debt_multiplier = U64F64::from_num(1) + Self::get_debt_rate(asset_id) * elapsed_time_U64F64;

        // 5 update pool index, supply, debt, timestamp
        let supply = TryInto::<u128>::try_into(pool.supply)
            .ok()
            .expect("Balance is u128");
        let supply = supply * supply_multiplier;
        let converted = u128::from_fixed(supply);
        pool.supply = TryInto::<T::Balance>::try_into(converted)
            .ok()
            .expect("Balance is u128");
        pool.total_supply_index *= supply_multiplier;

        let debt = TryInto::<u128>::try_into(pool.debt)
            .ok()
            .expect("Balance is u128");
        let debt = debt * debt_multiplier;
        let converted = u128::from_fixed(debt);
        pool.debt = TryInto::<T::Balance>::try_into(converted)
            .ok()
            .expect("Balance is u128");
        pool.total_debt_index *= debt_multiplier;

        Pools::<T>::insert(asset_id, pool);

    }

    fn get_debt_rate(asset_id: T::AssetId) -> U64F64 {
        let utilization_optimal = U64F64::from_num(1) / 2;
        let borrow_rate_net1 = U64F64::from_num(7) / 100;
        let borrow_rate_net2 = U64F64::from_num(14) / 100;
        let borrow_rate_zero = U64F64::from_num(5) / 100;
        let borrow_rate_optimal = U64F64::from_num(10) / 100;

        let pool = Self::pool(asset_id).unwrap();
        let debt = TryInto::<u128>::try_into(pool.debt)
            .ok()
            .expect("Balance is u128");
        let supply = TryInto::<u128>::try_into(pool.supply)
            .ok()
            .expect("Balance is u128");

        let utilization_ratio;
        if supply == 0 {
            utilization_ratio = U64F64::from_num(0);
        } else {
            utilization_ratio = U64F64::from_num(debt) / supply;
        }
        if (utilization_ratio <= utilization_optimal) {
            return utilization_ratio * borrow_rate_net1 / utilization_optimal + borrow_rate_zero;
        } else {
            return (utilization_ratio - utilization_optimal) * borrow_rate_net2 / (U64F64::from_num(1) - utilization_optimal) /  borrow_rate_optimal;
        }
    }

    fn get_supply_rate(asset_id: T::AssetId) -> U64F64 {
        let utilization_optimal = U64F64::from_num(1) / 2;
        let borrow_rate_net1 = U64F64::from_num(7) / 100 / 2000000;
        let borrow_rate_net2 = U64F64::from_num(14) / 100 / 2000000;
        let borrow_rate_zero = U64F64::from_num(5) / 100 / 2000000;
        let borrow_rate_optimal = U64F64::from_num(10) / 100 / 2000000;

        let pool = Self::pool(asset_id).unwrap();
        let debt = TryInto::<u128>::try_into(pool.debt)
            .ok()
            .expect("Balance is u128");
        let supply = TryInto::<u128>::try_into(pool.supply)
            .ok()
            .expect("Balance is u128");
        let utilization_ratio;
        if supply == 0 {
            utilization_ratio = U64F64::from_num(0);
        } else {
            utilization_ratio = U64F64::from_num(debt) / supply;
        }
        if (utilization_ratio <= utilization_optimal) {
            return (utilization_ratio * borrow_rate_net1 / utilization_optimal + borrow_rate_zero) * utilization_ratio;
        } else {
            return ((utilization_ratio - utilization_optimal) * borrow_rate_net2 / (U64F64::from_num(1) - utilization_optimal) /  borrow_rate_optimal) * utilization_ratio;
        }
    }

    // TODO: if final amount = 0, clean up
    // TODO: return the actual - value if clean up
    fn update_user_supply(asset_id: T::AssetId, account: T::AccountId, amount: T::Balance, positive: bool) {
        let pool = Self::pool(asset_id).unwrap();

        if let Some(mut user_supply) = Self::user_supply(asset_id, account.clone()) {
            let original_amount = TryInto::<u128>::try_into(user_supply.amount)
                .ok()
                .expect("Balance is u128");
            let amount_with_interest = U64F64::from_num(original_amount) * pool.total_supply_index / user_supply.index;
            let converted = u128::from_fixed(amount_with_interest);
            user_supply.amount = TryInto::<T::Balance>::try_into(converted)
                .ok()
                .expect("Balance is u128");

            user_supply.index = pool.total_supply_index;

            if positive {
                user_supply.amount += amount;
            } else {
                user_supply.amount -= amount;
            }
            UserSupplies::<T>::insert(asset_id, account, user_supply);
        } else {
            let user_supply = UserSupply::<T> {
                amount,
                index: pool.total_supply_index,
                as_collateral: true,
            };
            UserSupplies::<T>::insert(asset_id, account, user_supply);
        }

    }

    // TODO: if final amount = 0, clean up
    // TODO: return the actual - value if clean up
    fn update_user_debt(asset_id: T::AssetId, account: T::AccountId, amount: T::Balance, positive: bool) {
        let pool = Self::pool(asset_id).unwrap();

        if let Some(mut user_debt) = Self::user_debt(asset_id, account.clone()) {
            let original_amount = TryInto::<u128>::try_into(user_debt.amount)
                .ok()
                .expect("Balance is u128");
            let amount_with_interest = U64F64::from_num(original_amount) * pool.total_debt_index / user_debt.index;
            let converted = u128::from_fixed(amount_with_interest);
            user_debt.amount = TryInto::<T::Balance>::try_into(converted)
                .ok()
                .expect("Balance is u128");

            user_debt.index = pool.total_debt_index;

            if positive {
                user_debt.amount += amount;
            } else {
                user_debt.amount -= amount;
            }
            UserDebts::<T>::insert(asset_id, account, user_debt);
        } else {
            let user_debt = UserDebt::<T> {
                amount,
                index: pool.total_debt_index,
            };
            UserDebts::<T>::insert(asset_id, account, user_debt);
        }
    }

    fn update_pool_supply(asset_id: T::AssetId, amount: T::Balance, positive: bool) {
        // TODO: error handle
        let mut pool = Self::pool(asset_id).unwrap();
        if positive {
            pool.supply += amount;
        } else {
            pool.supply -= amount;
        }
        Pools::<T>::insert(asset_id, pool);
    }

    fn update_pool_debt(asset_id: T::AssetId, amount: T::Balance, positive: bool) {
        // TODO: error handle
        let mut pool = Self::pool(asset_id).unwrap();
        if positive {
            pool.debt += amount;
        } else {
            pool.debt -= amount;
        }
        Pools::<T>::insert(asset_id, pool);
    }

    // tmp
    fn update_apys(asset_id: T::AssetId) {

        const BASE: u64 = 1000000;
        const BLOCK_PER_YEAR: u64 = 10*60*24*365;

        let mut pool = Self::pool(asset_id).unwrap();
        let supply_rate = Self::get_supply_rate(asset_id);
        let debt_rate = Self::get_debt_rate(asset_id);
    
        let supply_apy = supply_rate * U64F64::from_num(BLOCK_PER_YEAR * BASE);
        let debt_apy = debt_rate * U64F64::from_num(BLOCK_PER_YEAR * BASE);

        pool.supply_apy = TryInto::<T::Balance>::try_into(u128::from_fixed(supply_apy))
            .ok()
            .expect("Balance is u128");

        pool.debt_apy = TryInto::<T::Balance>::try_into(u128::from_fixed(debt_apy))
            .ok()
            .expect("Balance is u128");

        Pools::<T>::insert(asset_id, pool);
    }

    fn _init_pool(id: T::AssetId, can_be_collateral: bool) {

        let pool = Pool::<T> {
            enabled: true,
            can_be_collateral,
            asset: id,
            supply: T::Balance::zero(),
            debt: T::Balance::zero(),
            liquidation_threshold: U64F64::from_num(12) / 5,
            supply_threshold: U64F64::from_num(3) / 2,
            liquidation_bonus: U64F64::from_num(1) / 2,
            total_supply_index: U64F64::from_num(1),
            total_debt_index: U64F64::from_num(1),
            last_updated: <frame_system::Module<T>>::block_number(),
            supply_apy: T::Balance::zero(), // tmp
            debt_apy: T::Balance::zero() // tmp
        };

        Pools::<T>::insert(id, pool);
    }

    fn _update_user(account: T::AccountId) {

        let mut supply_balance = T::Balance::zero();
        let mut borrow_limit = T::Balance::zero();
        for asset in Self::user_supply_set(account.clone()).into_iter() {
            let amount = Self::user_supply(asset, account.clone()).unwrap().amount;
            let price = <assets::Module<T>>::price(asset);
            supply_balance += amount * price / T::Balance::from(1000000);
            borrow_limit += amount * price / T::Balance::from(1000000) * T::Balance::from(10) / T::Balance::from(15);
        }

        let mut debt_balance = T::Balance::zero();
        for asset in Self::user_debt_set(account.clone()).into_iter() {
            let amount = Self::user_debt(asset, account.clone()).unwrap().amount;
            let price = <assets::Module<T>>::price(asset);
            debt_balance += amount * price / T::Balance::from(1000000);
        }

        let user = User::<T> {
            borrow_limit,
            supply_balance,
            debt_balance,
        };

        Users::<T>::insert(account, user);

    }

}

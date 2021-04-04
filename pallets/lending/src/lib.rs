#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    debug,
    decl_event, decl_module, decl_storage, decl_error, ensure,
    StorageMap, Parameter,
};
use sp_runtime::{
    FixedU128, FixedPointNumber, FixedPointOperand,
    DispatchResult as Result, RuntimeDebug, ModuleId,
    traits::{
        Member, AtLeast32BitUnsigned, AccountIdConversion, Zero
    }, 
};
use frame_system::{self as system, ensure_signed};
use sp_std::prelude::*;
use sp_std::{vec::Vec, convert::TryInto};
use codec::{Encode, Decode};
use traits::{Oracle, MultiAsset};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

const PALLET_ID: ModuleId = ModuleId(*b"Lending!");

/// The module's configuration trait.
pub trait Trait: frame_system::Trait {
    /// The units in which we record balances.
    type Balance: Member + Parameter + FixedPointOperand + AtLeast32BitUnsigned + Default + Copy;
    /// The arithmetic type of asset identifier.
    type AssetId: Parameter + AtLeast32BitUnsigned + Default + Copy;

    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;

    type Oracle: Oracle<Self::AssetId, FixedU128>;

    type MultiAsset: MultiAsset<Self::AccountId, Self::AssetId, Self::Balance>;
}

/// Pending atomic swap operation.
#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct Pool<T: Trait> {
    pub enabled: bool,

    pub can_be_collateral: bool,

	pub asset: T::AssetId,

	pub supply: T::Balance,

    pub debt: T::Balance,

    pub safe_factor: FixedU128,

    pub close_factor: FixedU128, // < 1

    pub discount_factor: FixedU128,

    pub total_supply_index: FixedU128,

    pub total_debt_index: FixedU128,

    pub last_updated: T::BlockNumber,

    pub utilization_factor: FixedU128,

    pub initial_interest_rate: FixedU128,
    
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct UserSupply<T: Trait> {
	/// Source of the swap.
	pub amount: T::Balance,
	/// Action of this swap.
    pub index: FixedU128,
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct UserDebt<T: Trait> {
	/// Source of the swap.
	pub amount: T::Balance,
	/// Action of this swap.
	pub index: FixedU128,
}

decl_event!(
    pub enum Event<T> where 
    <T as system::Trait>::AccountId,
    <T as Trait>::Balance,
    <T as Trait>::AssetId {

        Supplied(AssetId, AccountId, Balance),

        Withdrawn(AssetId, AccountId, Balance),

        Borrowed(AssetId, AccountId, Balance),

        Repaid(AssetId, AccountId, Balance),

        Liquidated(AssetId, AssetId, AccountId, AccountId, Balance, Balance),

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

        // TODO: use bit set
        pub UserSupplySet get(fn user_supply_set): map hasher(blake2_128_concat) T::AccountId => Vec<T::AssetId>;
        pub UserDebtSet get(fn user_debt_set): map hasher(blake2_128_concat) T::AccountId => Vec<T::AssetId>;

        pub LiquidationThreshold get(fn get_liquidation_threshold): FixedU128 = FixedU128::one();
    }

    add_extra_genesis {
        config(pools): Vec<T::AssetId>;

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
        PoolNotExist,
        AssetNotCollateral,
        UserNotExist,
        AboveLiquidationThreshold,
        BelowLiquidationThreshold,
        UserNoSupply,
        UserNoDebt, 
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
            debug::info!("Entering supply");
            let account = ensure_signed(origin)?;

            // check pool exists and get pool instance
            let mut pool = Self::pool(asset_id).ok_or(Error::<T>::PoolNotExist)?;
            // accrue pool interest
            Self::accrue_interest(&mut pool);
            // transfer asset
            T::MultiAsset::transfer(
                account.clone(),
                asset_id,
                Self::account_id(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;

            // update user supply
            Self::update_user_supply(&pool, asset_id, account.clone(), amount, true);
            // update pool supply
            Self::update_pool_supply(&mut pool, amount, true);

            Self::deposit_event(RawEvent::Supplied(asset_id, account.clone(), amount));

            // update user's supply asset set
            let mut assets = Self::user_supply_set(account.clone());
            if !assets.iter().any(|x| *x == asset_id) {
                assets.push(asset_id);
                UserSupplySet::<T>::insert(account, assets);
            }

            // commit pool change to storage
            Pools::<T>::insert(asset_id, pool);

            debug::info!("Leaving supply");
            Ok(())
        }

        #[weight = 1]
        fn withdraw(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            debug::info!("Entering withdraw");

            let account = ensure_signed(origin)?;

            // check pool exists and get pool instance
            let mut pool = Self::pool(asset_id).ok_or(Error::<T>::PoolNotExist)?;
            // accrue pool interest
            Self::accrue_interest(&mut pool);

            // accrue user's interest
            Self::accrue_supply_with_interest(&pool, asset_id, account.clone());

            // pre-check amount
            // supply can not be zero (if so it will be eliminated)
            let mut amount = amount;
            if let Some(user_supply) = Self::user_supply(asset_id, account.clone()) {
                if user_supply.amount < amount {
                    amount = user_supply.amount;
                }
            } else {
                Err(Error::<T>::UserNoSupply)?
            }

            // check collateral 
            let (_, converted_supply, converted_borrow) = Self::get_user_info(account.clone());
            let price = T::Oracle::get_rate(asset_id);
            let converted_supply = converted_supply - (price * pool.safe_factor).saturating_mul_int(amount);
            ensure!(Self::get_liquidation_threshold().saturating_mul_int(converted_borrow) <= converted_supply, Error::<T>::BelowLiquidationThreshold);

            // check pool cash = (deposit - borrow) > amount
            if (pool.supply - pool.debt) < amount {
                Err(Error::<T>::NotEnoughLiquidity)?
            }

            // transfer asset to user
            T::MultiAsset::transfer(
                Self::account_id(),
                asset_id,
                account.clone(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;

            // update user supply
            Self::update_user_supply(&pool, asset_id, account.clone(), amount, false);

            // update pool supply
            Self::update_pool_supply(&mut pool, amount, false);

            Self::deposit_event(RawEvent::Withdrawn(asset_id, account, amount));

            // commit pool change to storage
            Pools::<T>::insert(asset_id, pool);

            debug::info!("Leaving withdraw");
            Ok(())
        }

        #[weight = 1]
        fn borrow(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            debug::info!("Entering borrow");
            let account = ensure_signed(origin)?;

            // check pool exists and get pool instance
            let mut pool = Self::pool(asset_id).ok_or(Error::<T>::PoolNotExist)?;

            // accrue interest
            Self::accrue_interest(&mut pool);

            // check pool cash = (deposit - borrow) > amount
            if (pool.supply - pool.debt) < amount {
                Err(Error::<T>::NotEnoughLiquidity)?
            }

            // need to accrue user interest first
            Self::accrue_debt_with_interest(&pool, asset_id, account.clone());

            // check collateral
            let (_, converted_supply, converted_borrow) = Self::get_user_info(account.clone());
            let price = T::Oracle::get_rate(asset_id);
            let converted_borrow = converted_borrow + price.saturating_mul_int(amount);
            ensure!(Self::get_liquidation_threshold().saturating_mul_int(converted_borrow) <= converted_supply, Error::<T>::BelowLiquidationThreshold);

            // transfer asset to user
            T::MultiAsset::transfer(
                Self::account_id(),
                asset_id,
                account.clone(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;
            // update user Borrow
            Self::update_user_debt(&pool, asset_id, account.clone(), amount, true);
            // update pool borrow
            Self::update_pool_debt(&mut pool, amount, true);

            Self::deposit_event(RawEvent::Borrowed(asset_id, account.clone(), amount));

            // update user's debt asset set
            let mut assets = Self::user_debt_set(account.clone());
            if !assets.iter().any(|x| *x == asset_id) {
                assets.push(asset_id);
                UserDebtSet::<T>::insert(account, assets);
            }

            // commit pool change to storage
            Pools::<T>::insert(asset_id, pool);
            debug::info!("Leaving borrow");

            Ok(())
        }

        #[weight = 1]
        fn repay(
            origin,
            asset_id: T::AssetId,
            amount: T::Balance) -> Result {
            debug::info!("Entering repay");

            let account = ensure_signed(origin)?;

            // check pool exists and get pool instance
            let mut pool = Self::pool(asset_id).ok_or(Error::<T>::PoolNotExist)?;
            // accrue interest
            Self::accrue_interest(&mut pool);

            // accrue user's interest
            Self::accrue_debt_with_interest(&pool, asset_id, account.clone());

            // pre-check amount
            let mut amount = amount;
            if let Some(user_debt) = Self::user_debt(asset_id, account.clone()) {
                if user_debt.amount < amount {
                    amount = user_debt.amount;
                }
            } else {
                Err(Error::<T>::UserNoDebt)?
            }

            // transfer token from user
            T::MultiAsset::transfer(
                account.clone(),
                asset_id,
                Self::account_id(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;

            // update user Borrow
            Self::update_user_debt(&pool, asset_id, account.clone(), amount, false);
            // update pool borrow
            Self::update_pool_debt(&mut pool, amount, false);

            Self::deposit_event(RawEvent::Repaid(asset_id, account, amount));

            // commit pool change to storage
            Pools::<T>::insert(asset_id, pool);
            debug::info!("Leaving repay");

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
            debug::info!("Entering liquidate");
            let account = ensure_signed(origin)?;

            // check pool exists and get pool instances
            // check if get_asset_id is enabled as collateral
            let mut get_pool = Self::pool(get_asset_id).ok_or(Error::<T>::PoolNotExist)?;
            ensure!(get_pool.can_be_collateral, Error::<T>::AssetNotCollateral);
            
            let mut pay_pool = Self::pool(pay_asset_id).ok_or(Error::<T>::PoolNotExist)?;

            // 2 accrue interest of pay and get asset
            Self::accrue_interest(&mut pay_pool);
            Self::accrue_interest(&mut get_pool);

            // accrue target user's interest
            Self::accrue_supply_with_interest(&get_pool, get_asset_id, target_user.clone());
            Self::accrue_debt_with_interest(&pay_pool, pay_asset_id, target_user.clone());
            
            // 3 check if target user is under liquidation condition
            let (_, converted_supply, converted_borrow) = Self::get_user_info(target_user.clone());
            ensure!(Self::get_liquidation_threshold().saturating_mul_int(converted_borrow) > converted_supply, Error::<T>::AboveLiquidationThreshold);
        
            // 4 check if liquidation % is more than threshold 
            // TODO: if target user supply is too small, enable total liquidation
            let target_user_supply = Self::user_supply(get_asset_id, target_user.clone()).ok_or(Error::<T>::UserNoSupply)?;
            let get_limit = get_pool.close_factor.saturating_mul_int(target_user_supply.amount);
            
            let get_price = T::Oracle::get_rate(get_asset_id);
            let pay_price = T::Oracle::get_rate(pay_asset_id);
            let pay_limit = (get_price / pay_price * get_pool.discount_factor).saturating_mul_int(get_limit);
            let target_user_debt = Self::user_debt(pay_asset_id, target_user.clone()).ok_or(Error::<T>::UserNoSupply)?;

            let mut pay_asset_amount = pay_asset_amount;

            if pay_asset_amount > pay_limit {
                pay_asset_amount = pay_limit;
            }

            if pay_asset_amount > target_user_debt.amount {
                pay_asset_amount = target_user_debt.amount;
            }

            // TODO: check rounding errors due to discount_factor
            let get_asset_amount = (pay_price / get_price / get_pool.discount_factor ).saturating_mul_int(pay_asset_amount);

            // 5 transfer token from arbitrager
            T::MultiAsset::transfer(
                account.clone(),
                pay_asset_id,
                Self::account_id(),
                pay_asset_amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;            

            // 6 transfer collateral to arbitrager
            T::MultiAsset::transfer(
                Self::account_id(),
                get_asset_id,
                account.clone(),
                get_asset_amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;
            // 7 recalculate target user's borrow and supply in 2 pools
            Self::update_user_supply(&mut get_pool, get_asset_id, target_user.clone(), get_asset_amount, false);
            Self::update_user_debt(&mut pay_pool, pay_asset_id, target_user, pay_asset_amount, false);

            // update pools
            Pools::<T>::insert(get_asset_id, get_pool);
            Pools::<T>::insert(pay_asset_id, pay_pool);

            debug::info!("Leaving liquidate");

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

impl<T: Trait> Module<T> where
{

    fn account_id() -> T::AccountId {
		PALLET_ID.into_account()
    }
    
    fn accrue_interest(pool: &mut Pool<T>) {
        debug::info!("Entering accrue_interest");

        if pool.last_updated == <frame_system::Module<T>>::block_number() {
            debug::info!("Leaving accrue_interest");
            return
        }

        // get time span
        let interval_block_number = <frame_system::Module<T>>::block_number() - pool.last_updated;
		let elapsed_time_u32 = TryInto::<u32>::try_into(interval_block_number)
			.ok()
			.expect("blockchain will not exceed 2^32 blocks; qed");

        // get rates and calculate interest
        let supply_multiplier = FixedU128::one() + Self::supply_rate_internal(pool) * FixedU128::saturating_from_integer(elapsed_time_u32);
        let debt_multiplier = FixedU128::one() + Self::debt_rate_internal(pool) * FixedU128::saturating_from_integer(elapsed_time_u32);

        pool.supply = supply_multiplier.saturating_mul_int(pool.supply);
        pool.total_supply_index = pool.total_supply_index * supply_multiplier;

        pool.debt = debt_multiplier.saturating_mul_int(pool.debt);
        pool.total_debt_index = pool.total_debt_index * debt_multiplier;

        pool.last_updated = <frame_system::Module<T>>::block_number();
        debug::info!("Leaving accrue_interest");

    }

    // amount is pre-checked so will no be negative
    fn update_user_supply(pool: &Pool<T>, asset_id: T::AssetId, account: T::AccountId, amount: T::Balance, positive: bool) {
        debug::info!("Entering update_user_supply");
        if let Some(mut user_supply) = Self::user_supply(asset_id, account.clone()) {

            user_supply.amount = (pool.total_supply_index / user_supply.index).saturating_mul_int(user_supply.amount);

            user_supply.index = pool.total_supply_index;

            if positive {
                user_supply.amount += amount;
            } else {
                user_supply.amount -= amount;
            }
            if user_supply.amount != T::Balance::zero() {
                UserSupplies::<T>::insert(asset_id, account, user_supply);
            } else {
                UserSupplies::<T>::remove(asset_id, account.clone());
                // update user's supply asset set
                let mut assets = Self::user_supply_set(account.clone());
                assets.retain(|x| *x != asset_id);
                UserSupplySet::<T>::insert(account, assets);
            }
        } else if amount != T::Balance::zero() {
            let user_supply = UserSupply::<T> {
                amount,
                index: pool.total_supply_index,
            };
            UserSupplies::<T>::insert(asset_id, account, user_supply);
        }
        debug::info!("Leaving update_user_supply");

    }

    // amount is pre-checked so will no be negative
    fn update_user_debt(pool: &Pool<T>, asset_id: T::AssetId, account: T::AccountId, amount: T::Balance, positive: bool) {
        debug::info!("Entering update_user_debt");

        if let Some(mut user_debt) = Self::user_debt(asset_id, account.clone()) {
            user_debt.amount = (pool.total_debt_index / user_debt.index).saturating_mul_int(user_debt.amount);

            user_debt.index = pool.total_debt_index;

            if positive {
                user_debt.amount += amount;
            } else {
                user_debt.amount -= amount;
            }
            if user_debt.amount != T::Balance::zero() {
                UserDebts::<T>::insert(asset_id, account, user_debt);
            } else {
                UserDebts::<T>::remove(asset_id, account.clone());
                // update user's debt asset set
                let mut assets = Self::user_debt_set(account.clone());
                assets.retain(|x| *x != asset_id);
                UserDebtSet::<T>::insert(account, assets);
            }
        } else if amount != T::Balance::zero() {
            let user_debt = UserDebt::<T> {
                amount,
                index: pool.total_debt_index,
            };
            UserDebts::<T>::insert(asset_id, account, user_debt);
        }
        debug::info!("Leaving update_user_debt");

    }

    fn update_pool_supply(pool: &mut Pool<T>, amount: T::Balance, positive: bool) {
        debug::info!("Entering update_pool_supply");

        if positive {
            pool.supply += amount;
        } else {
            pool.supply -= amount;
        }
        debug::info!("Leaving update_pool_supply");
    }

    fn update_pool_debt(pool: &mut Pool<T>, amount: T::Balance, positive: bool) {
        debug::info!("Entering update_pool_debt");
        if positive {
            pool.debt += amount;
        } else {
            pool.debt -= amount;
        }
        debug::info!("Leaving update_pool_debt");

    }

    fn _init_pool(id: T::AssetId, can_be_collateral: bool) {

        let pool = Pool::<T> {
            enabled: true,
            can_be_collateral,
            asset: id,
            supply: T::Balance::zero(),
            debt: T::Balance::zero(),
            safe_factor: FixedU128::saturating_from_rational(7, 10),
            close_factor: FixedU128::one(),
            discount_factor: FixedU128::saturating_from_rational(95, 100),
            total_supply_index: FixedU128::one(),
            total_debt_index: FixedU128::one(),
            last_updated: <frame_system::Module<T>>::block_number(),
            utilization_factor: FixedU128::saturating_from_rational(385, 10000000000u64),
            initial_interest_rate: FixedU128::saturating_from_rational(385, 100000000000u64),
        };

        Pools::<T>::insert(id, pool);
    }

    fn supply_rate_internal(pool: &Pool<T>) -> FixedU128 {
        debug::info!("Entering supply_rate_internal");

        if pool.supply == T::Balance::zero() {
            debug::info!("Leaving supply_rate_internal");

            return FixedU128::zero();
        } 

        let utilization_ratio = FixedU128::saturating_from_rational(pool.debt, pool.supply);
        debug::info!("Leaving supply_rate_internal");
        let debt_rate = Self::debt_rate_internal(pool);
        let supply_rate = debt_rate * utilization_ratio;
        return supply_rate;
    }

    fn debt_rate_internal(pool: &Pool<T>) -> FixedU128 {
        debug::info!("Entering debt_rate_internal");

        if pool.supply == T::Balance::zero() {
            debug::info!("Leaving debt_rate_internal");
            return pool.initial_interest_rate;
        } 

        let utilization_ratio = FixedU128::saturating_from_rational(pool.debt, pool.supply);
        debug::info!("Leaving debt_rate_internal");
        pool.initial_interest_rate + pool.utilization_factor * utilization_ratio

    }

    /// runtime apis
    pub fn supply_rate(id: T::AssetId) -> FixedU128 {
        debug::info!("Entering supply_rate");

        let pool = Self::pool(id);
        if pool.is_none() {
            debug::info!("Leaving supply_rate");

            return FixedU128::zero()
        }

        let pool = pool.unwrap();
        debug::info!("Leaving supply_rate");

        Self::supply_rate_internal(&pool)

    }

    pub fn debt_rate(id: T::AssetId) -> FixedU128 {
        debug::info!("Entering debt_rate");

        let pool = Self::pool(id);
        if pool.is_none() {
            debug::info!("Leaving debt_rate");

            return FixedU128::zero()
        }

        let pool = pool.unwrap();
        debug::info!("Leaving debt_rate");

        Self::debt_rate_internal(&pool)
    }

    // total supply balance; total converted supply balance; total debt balance;
    pub fn get_user_info(user: T::AccountId) -> (T::Balance, T::Balance, T::Balance) {
        debug::info!("Entering get_user_info");
        let mut supply_balance = T::Balance::zero();
        let mut supply_converted = T::Balance::zero();
        for asset in Self::user_supply_set(user.clone()).into_iter() {
            let amount = Self::get_user_supply_with_interest(asset, user.clone());
            let price = T::Oracle::get_rate(asset);
            supply_balance += price.saturating_mul_int(amount);
            // TODO: optimize this
            supply_converted += (price * Self::pool(asset).unwrap().safe_factor).saturating_mul_int(amount);
        }

        let mut debt_balance = T::Balance::zero();
        for asset in Self::user_debt_set(user.clone()).into_iter() {
            let amount = Self::get_user_debt_with_interest(asset, user.clone());
            let price = T::Oracle::get_rate(asset);
            debt_balance += price.saturating_mul_int(amount);
        }
        debug::info!("Leaving get_user_info");

        (supply_balance, supply_converted, debt_balance)
    }

    pub fn get_user_debt_with_interest(asset_id: T::AssetId, user: T::AccountId) -> T::Balance {
        debug::info!("Entering get_user_debt_with_interest");
        let total_debt_index;

        if let Some(pool) = Self::pool(asset_id) {
            let interval_block_number = <frame_system::Module<T>>::block_number() - pool.last_updated;
            let elapsed_time_u32 = TryInto::<u32>::try_into(interval_block_number)
                .ok()
                .expect("blockchain will not exceed 2^32 blocks; qed");
    
            let debt_multiplier = FixedU128::one() + Self::debt_rate_internal(&pool) * FixedU128::saturating_from_integer(elapsed_time_u32);
            total_debt_index = pool.total_debt_index * debt_multiplier;

        } else {
            debug::info!("Leaving get_user_debt_with_interest");
            return T::Balance::zero()
        }

        debug::info!("Leaving get_user_debt_with_interest");
        if let Some(user_debt) = Self::user_debt(asset_id, user) {
            (total_debt_index / user_debt.index).saturating_mul_int(user_debt.amount)
        } else {
            T::Balance::zero()
        }
    }

    pub fn get_user_supply_with_interest(asset_id: T::AssetId, user: T::AccountId) -> T::Balance {
        debug::info!("Entering get_user_supply_with_interest");

        let total_supply_index;

        if let Some(pool) = Self::pool(asset_id) {
            let interval_block_number = <frame_system::Module<T>>::block_number() - pool.last_updated;
            let elapsed_time_u32 = TryInto::<u32>::try_into(interval_block_number)
                .ok()
                .expect("blockchain will not exceed 2^32 blocks; qed");
    
            let supply_multiplier = FixedU128::one() + Self::supply_rate_internal(&pool) * FixedU128::saturating_from_integer(elapsed_time_u32);
            total_supply_index = pool.total_supply_index * supply_multiplier;

        } else {
            debug::info!("Leaving get_user_supply_with_interest");
            return T::Balance::zero()
        }
        debug::info!("Leaving get_user_supply_with_interest");

        if let Some(user_supply) = Self::user_supply(asset_id, user) {
            (total_supply_index / user_supply.index).saturating_mul_int(user_supply.amount)
        } else {
            T::Balance::zero()
        }
    }

    // pool interest is already accrued
    fn accrue_debt_with_interest(pool: &Pool<T>, asset_id: T::AssetId, user: T::AccountId) {
        debug::info!("Entering accrue_debt_with_interest");

        if let Some(mut user_debt) = Self::user_debt(asset_id, user.clone()) {
            user_debt.amount = (pool.total_debt_index / user_debt.index).saturating_mul_int(user_debt.amount);
            user_debt.index = pool.total_debt_index;
            UserDebts::<T>::insert(asset_id, user, user_debt);
        } 
        debug::info!("Leaving accrue_debt_with_interest");

    }

    // pool interest is already accrued
    fn accrue_supply_with_interest(pool: &Pool<T>, asset_id: T::AssetId, user: T::AccountId) {
        debug::info!("Entering accrue_supply_with_interest");

        if let Some(mut user_supply) = Self::user_supply(asset_id, user.clone()) {
            user_supply.amount = (pool.total_supply_index / user_supply.index).saturating_mul_int(user_supply.amount);
            user_supply.index = pool.total_supply_index;
            UserSupplies::<T>::insert(asset_id, user, user_supply);
        }
        debug::info!("Leaving accrue_supply_with_interest");

    }

}

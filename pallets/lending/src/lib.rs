#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_event, decl_module, decl_storage, decl_error,
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

// TODO: fee, reserves
// TODO: child storage
// TODO: reduce pool storage read

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

    pub liquidation_factor: FixedU128,

    pub safe_factor: FixedU128,

    pub liquidation_bonus: FixedU128,

    pub total_supply_index: FixedU128,

    pub total_debt_index: FixedU128,

    pub last_updated: T::BlockNumber,
    
}

#[derive(Clone, Eq, PartialEq, RuntimeDebug, Encode, Decode)]
pub struct UserSupply<T: Trait> {
	/// Source of the swap.
	pub amount: T::Balance,
	/// Action of this swap.
    pub index: FixedU128,
    
    pub as_collateral: bool,
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
            T::MultiAsset::transfer(
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
                UserSupplySet::<T>::insert(account, assets);
            }

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
            // 2 check collateral TODO need to first update user's supply to check the latest ratio...
            
            // pre-check amount
            // TODO: what if user supply is zero?
            let mut amount = amount;
            if let Some(user_supply) = Self::user_supply(asset_id, account.clone()) {
                if user_supply.amount < amount {
                    amount = user_supply.amount;
                }
            }

            // 3 check pool cash = (deposit - borrow) > amount
            let pool = Self::pool(asset_id).unwrap();
            if (pool.supply - pool.debt) < amount {
                Err(Error::<T>::NotEnoughLiquidity)?
            }

            // 4 transfer asset to user
            T::MultiAsset::transfer(
                Self::account_id(),
                asset_id,
                account.clone(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;

            // 5 update user supply
            Self::update_user_supply(asset_id, account.clone(), amount, false);

            // 6 update pool supply
            Self::update_pool_supply(asset_id, amount, false);

            Self::deposit_event(RawEvent::Withdrawn(asset_id, account, amount));

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
            T::MultiAsset::transfer(
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
                UserDebtSet::<T>::insert(account, assets);
            }
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
            T::MultiAsset::transfer(
                account.clone(), // TODO: use reference
                asset_id,
                Self::account_id(),
                amount,
            ).map_err(|_| Error::<T>::TransferFailed)?;

            // pre-check amount
            let mut amount = amount;
            if let Some(user_debt) = Self::user_debt(asset_id, account.clone()) {
                if user_debt.amount < amount {
                    amount = user_debt.amount;
                }
            }

            // 3 update user Borrow
            Self::update_user_debt(asset_id, account.clone(), amount, false);
            // 4 update pool borrow
            Self::update_pool_debt(asset_id, amount, false);

            Self::deposit_event(RawEvent::Repaid(asset_id, account, amount));

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

impl<T: Trait> Module<T> where
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
		let elapsed_time_u32 = TryInto::<u32>::try_into(interval_block_number)
			.ok()
			.expect("blockchain will not exceed 2^32 blocks; qed");

        // 4  get rates and calculate interest
        let supply_multiplier = FixedU128::saturating_from_integer(1) + Self::supply_rate(asset_id) * FixedU128::saturating_from_integer(elapsed_time_u32);
        let debt_multiplier = FixedU128::saturating_from_integer(1) + Self::debt_rate(asset_id) * FixedU128::saturating_from_integer(elapsed_time_u32);

        pool.supply = supply_multiplier.saturating_mul_int(pool.supply);
        pool.total_supply_index = pool.total_supply_index * supply_multiplier;

        pool.debt = debt_multiplier.saturating_mul_int(pool.debt);
        pool.total_debt_index = pool.total_debt_index * debt_multiplier;

        Pools::<T>::insert(asset_id, pool);

    }

    // TODO: if final amount = 0, clean up
    // TODO: return the actual - value if clean up
    fn update_user_supply(asset_id: T::AssetId, account: T::AccountId, amount: T::Balance, positive: bool) {
        let pool = Self::pool(asset_id).unwrap();

        if let Some(mut user_supply) = Self::user_supply(asset_id, account.clone()) {

            user_supply.amount = (pool.total_supply_index / user_supply.index).saturating_mul_int(user_supply.amount);

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
            user_debt.amount = (pool.total_debt_index / user_debt.index).saturating_mul_int(user_debt.amount);

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

    fn _init_pool(id: T::AssetId, can_be_collateral: bool) {

        let pool = Pool::<T> {
            enabled: true,
            can_be_collateral,
            asset: id,
            supply: T::Balance::zero(),
            debt: T::Balance::zero(),
            liquidation_factor: FixedU128::saturating_from_rational(120, 100),
            safe_factor: FixedU128::saturating_from_rational(150, 100),
            liquidation_bonus: FixedU128::saturating_from_rational(105, 100),
            total_supply_index: FixedU128::saturating_from_integer(1),
            total_debt_index: FixedU128::saturating_from_integer(1),
            last_updated: <frame_system::Module<T>>::block_number(),
        };

        Pools::<T>::insert(id, pool);
    }

    /// runtime apis
    pub fn supply_rate(id: T::AssetId) -> FixedU128 {
        let utilization_optimal = FixedU128::saturating_from_rational(1, 2);
        let borrow_rate_net1 = FixedU128::saturating_from_rational(7, 100*2000000);
        let borrow_rate_net2 = FixedU128::saturating_from_rational(14, 100*2000000);
        let borrow_rate_zero = FixedU128::saturating_from_rational(5, 100*2000000);
        let borrow_rate_optimal = FixedU128::saturating_from_rational(10, 100*2000000);

        let pool = Self::pool(id).unwrap();

        let utilization_ratio;
        if pool.supply == T::Balance::zero() {
            utilization_ratio = FixedU128::zero();
        } else {
            utilization_ratio = FixedU128::saturating_from_rational(pool.debt, pool.supply);
        }
        if utilization_ratio <= utilization_optimal {
            return utilization_ratio * borrow_rate_net1 / utilization_optimal + borrow_rate_zero;
        } else {
            return (utilization_ratio - utilization_optimal) * borrow_rate_net2 / (FixedU128::saturating_from_integer(1) - utilization_optimal) +  borrow_rate_optimal;
        }
    }

    pub fn debt_rate(id: T::AssetId) -> FixedU128 {
        let utilization_optimal = FixedU128::saturating_from_rational(1, 2);
        let borrow_rate_net1 = FixedU128::saturating_from_rational(7, 100*2000000);
        let borrow_rate_net2 = FixedU128::saturating_from_rational(14, 100*2000000);
        let borrow_rate_zero = FixedU128::saturating_from_rational(5, 100*2000000);
        let borrow_rate_optimal = FixedU128::saturating_from_rational(10, 100*2000000);

        let pool = Self::pool(id).unwrap();

        let utilization_ratio;
        if pool.supply == T::Balance::zero() {
            utilization_ratio = FixedU128::zero();
        } else {
            utilization_ratio = FixedU128::saturating_from_rational(pool.debt, pool.supply);
        }
        if utilization_ratio <= utilization_optimal {
            return (utilization_ratio * borrow_rate_net1 / utilization_optimal + borrow_rate_zero) * utilization_ratio;
        } else {
            return ((utilization_ratio - utilization_optimal) * borrow_rate_net2 / (FixedU128::saturating_from_integer(1) - utilization_optimal) +  borrow_rate_optimal) * utilization_ratio;
        }
    }

    // effective borrow limit; debt balance
    pub fn get_user_info(user: T::AccountId) -> (T::Balance, T::Balance) {
        let mut supply_balance = T::Balance::zero();
        let mut borrow_limit = T::Balance::zero();
        for asset in Self::user_supply_set(user.clone()).into_iter() {
            let amount = Self::user_supply(asset, user.clone()).unwrap().amount;
            let price = T::Oracle::get_rate(asset);
            supply_balance += price.saturating_mul_int(amount);
            borrow_limit += price.saturating_mul_int(amount) * T::Balance::from(10) / T::Balance::from(15);
        }

        let mut debt_balance = T::Balance::zero();
        for asset in Self::user_debt_set(user.clone()).into_iter() {
            let amount = Self::user_debt(asset, user.clone()).unwrap().amount;
            let price = T::Oracle::get_rate(asset);
            debt_balance += price.saturating_mul_int(amount);
        }


        (borrow_limit, debt_balance)
    }
}

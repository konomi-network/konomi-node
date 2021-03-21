#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
    decl_event, decl_module, decl_storage, ensure, Parameter,
    StorageMap, StorageValue,
};
use sp_runtime::{
    FixedU128, FixedPointNumber,
    DispatchResult as Result,
    FixedPointOperand,
};
use frame_system::{self as system, ensure_signed, ensure_root};
use sp_std::prelude::*;
use sp_std::convert::TryInto;
use sp_runtime::traits::{
    Member, One, AtLeast32BitUnsigned, Zero,
};
use traits::{Oracle, MultiAsset};

/// The module configuration trait.
pub trait Trait: frame_system::Trait {
    /// The overarching event type.
    type Event: From<Event<Self>> + Into<<Self as system::Trait>::Event>;
    /// The units in which we record balances.
    type Balance: Member + Parameter + FixedPointOperand + AtLeast32BitUnsigned + Default + Copy;
    /// The arithmetic type of asset identifier.
    type AssetId: Parameter + AtLeast32BitUnsigned + Default + Copy;
}

decl_module! {
    pub struct Module<T: Trait> for enum Call where
        origin: T::Origin
    {
		fn deposit_event() = default;

        #[weight = 1]
        fn issue(origin, total: T::Balance) -> Result {
            let origin = ensure_signed(origin)?;

            let id = Self::next_asset_id();
            <NextAssetId<T>>::mutate(|id| *id += One::one());

            <Balances<T>>::insert((id, origin.clone()), total);
            <TotalSupply<T>>::insert(id, total);

            // debug
            sp_runtime::print("----> asset id, total balance");
            let idn = TryInto::<u64>::try_into(id)
                .ok()
                .expect("id is u64");
            sp_runtime::print(idn);
            let b = TryInto::<u128>::try_into(<Balances<T>>::get((id, origin.clone())))
                .ok()
                .expect("Balance is u128");
            sp_runtime::print(b as u64);

            Self::deposit_event(RawEvent::Issued(id, origin, total));

            Ok(())
        }

        /// Destroy any assets of `id` owned by `origin`.
        /// @origin
        /// @id      Asset id to be destroyed
        #[weight = 1]
        fn destroy(origin, id: T::AssetId) -> Result {
            let origin = ensure_signed(origin)?;
            let balance = <Balances<T>>::take((id, origin.clone()));
            ensure!(!balance.is_zero(), "origin balance should be non-zero");

            <TotalSupply<T>>::mutate(id, |total_supply| *total_supply -= balance);
            Self::deposit_event(RawEvent::Destroyed(id, origin, balance));

            Ok(())
        }

        /// Set the default inherent asset
        /// @origin
        /// @asset    The asset to become inherent asset
        #[weight = 1]
        pub fn set_inherent_asset(origin, asset: T::AssetId) -> Result {
            //ensure_root(origin)?;
            <InherentAsset<T>>::mutate(|ia| *ia = asset.clone());

            // debug
            sp_runtime::print("----> Inhere Asset Id");
            
            let idn = TryInto::<u64>::try_into(Self::inherent_asset_id())
                .ok()  
                .expect("id is u64");        
            sp_runtime::print(idn);

            Ok(())
        }

        /// Transfer an asset to another account
        #[weight = 1]
        pub fn transfer_asset(origin,
                    id: T::AssetId,
                    to_account: T::AccountId,
                    amount: T::Balance
        ) -> Result {
            let from_account = ensure_signed(origin)?;
            Self::transfer(from_account, id, to_account, amount);

            Ok(())
        }

        #[weight = 1]
        pub fn set_price(origin, id: T::AssetId, price: FixedU128) -> Result {
            ensure_root(origin)?;
            Self::_set_price(id, price);
            Ok(())
        }
	}
}

decl_event! {
	pub enum Event<T> where
		<T as frame_system::Trait>::AccountId,
		<T as Trait>::Balance,
		<T as Trait>::AssetId,
	{
		/// Some assets were issued. \[asset_id, owner, total_supply\]
		Issued(AssetId, AccountId, Balance),
		/// Some assets were transferred. \[asset_id, from, to, amount\]
		Transferred(AssetId, AccountId, AccountId, Balance),
		/// Some assets were destroyed. \[asset_id, owner, balance\]
		Destroyed(AssetId, AccountId, Balance),
	}
}

decl_storage! {
    trait Store for Module<T: Trait> as Assets
    {
        /// The next asset identifier up for grabs.
        NextAssetId get(fn next_asset_id): T::AssetId;
        /// The total unit supply of an asset.
        TotalSupply get(fn get_asset_total_supply): map hasher(twox_64_concat) T::AssetId => T::Balance;
        /// The number of units of assets held by any given account.
        Balances get(fn get_asset_balance): map hasher(blake2_128_concat) (T::AssetId, T::AccountId) => T::Balance;
        /// The default inherent asset in this platform
        InherentAsset get(fn inherent_asset_id): T::AssetId;
        /// Price of the asset (base is 1000000)
        Price get(fn price): map hasher(twox_64_concat) T::AssetId => FixedU128;
        /// for test only
        Owner get(fn owner) config(): T::AccountId;
    }
    
    add_extra_genesis {
        config(assets): Vec<(T::AccountId, T::Balance, u64)>;

        build(|config: &GenesisConfig<T>| {
            for asset in config.assets.iter() {
                let (account, amount, price) = asset;
                <Module<T>>::_issue(account.clone(), amount.clone());
                let to_account = <Owner<T>>::get();
                let asset_id = <NextAssetId<T>>::get() - 1u32.into();
                <Module<T>>::transfer(account.clone(), asset_id, to_account, 500000u32.into());
                <Module<T>>::_set_price(asset_id, FixedU128::saturating_from_integer(*price));
            }
        })
    }
}

impl<T: Trait> Module<T> {
    /// Issue a new class of fungible assets. There are, and will only ever be, `total`
    /// such assets and they'll all belong to the `origin` initially. It will have an
    /// identifier `AssetId` instance: this will be specified in the `Issued` event.
    /// This will make a increased id asset.
    /// @origin
    /// @total    How much balance of new asset
    fn _issue(account: T::AccountId, total: T::Balance) -> sp_std::result::Result<(), &'static str> {
        let id = Self::next_asset_id();
        <NextAssetId<T>>::mutate(|id| *id += One::one());

        <Balances<T>>::insert((id, account.clone()), total);
        <TotalSupply<T>>::insert(id, total);

        // debug
        sp_runtime::print("----> asset id, total balance");
        let idn = TryInto::<u64>::try_into(id)
            .ok()
            .expect("id is u64");        
        sp_runtime::print(idn);
        let b = TryInto::<u128>::try_into(<Balances<T>>::get((id, account.clone())))
            .ok()
            .expect("Balance is u128");        
        sp_runtime::print(b as u64);

        Self::deposit_event(RawEvent::Issued(id, account, total));

        Ok(())
    }

    pub fn _set_price(id: T::AssetId, price: FixedU128) {
        <Price<T>>::insert(id, price);
    }

    /// Move some assets from one holder to another.
    /// @from_account    The account lost amount of a certain asset balance
    /// @id              The asset id to be transfered
    /// @to_account      The account receive the sent asset balance
    /// @amount          The amount value to be transfered
    pub fn transfer(
        from_account: T::AccountId,
        id: T::AssetId,
        to_account: T::AccountId,
        amount: T::Balance,
    ) -> sp_std::result::Result<(), &'static str> {
        let origin_account = (id, from_account.clone());
        let origin_balance = <Balances<T>>::get(&origin_account);
        let target = to_account;
        ensure!(!amount.is_zero(), "transfer amount should be non-zero");
        ensure!(
            origin_balance >= amount,
            "origin account balance must be greater than or equal to the transfer amount"
        );

        Self::deposit_event(RawEvent::Transferred(
            id,
            from_account,
            target.clone(),
            amount,
        ));

        sp_runtime::print("before transfer target balance ----> ");
        let b = TryInto::<u128>::try_into(Self::get_asset_balance(&(id.clone(), target.clone())))
            .ok()
            .expect("Balance is u128");
        sp_runtime::print(b as u64);
        <Balances<T>>::insert(origin_account, origin_balance - amount);
        <Balances<T>>::mutate((id, target.clone()), |balance| *balance += amount);
        sp_runtime::print("after transfer target balance----> ");
        let b = TryInto::<u128>::try_into(Self::get_asset_balance(&(id.clone(), target)))
            .ok()
            .expect("Balance is u128");
        sp_runtime::print(b as u64);
        Ok(())
    }

    /// Get the asset `id` balance of `who`.
    /// @id    Asset id
    /// @who   Account id
    pub fn balance(id: T::AssetId, who: T::AccountId) -> T::Balance {
        // debug
        sp_runtime::print("----> Account Asset Balance");
        let b = TryInto::<u128>::try_into(Self::get_asset_balance(&(id.clone(), who.clone())))
            .ok()
            .expect("Balance is u128");
        sp_runtime::print(b as u64);

        <Balances<T>>::get((id, who))
    }

    /// Get the total supply of an asset `id`.
    /// @id    Asset id
    pub fn total_supply(id: T::AssetId) -> T::Balance {
        // debug
        sp_runtime::print("----> Asset Total Supply");
        let b = TryInto::<u128>::try_into(Self::get_asset_total_supply(id.clone()))
            .ok()
            .expect("Balance is u128");
        sp_runtime::print(b as u64);

        <TotalSupply<T>>::get(id)
    }

}

impl<T: Trait> Oracle<T::AssetId, FixedU128> for Module<T> {
    fn get_rate(asset_id: T::AssetId) -> FixedU128 {
        Self::price(asset_id)
    }
}

impl<T: Trait> MultiAsset<T::AccountId, T::AssetId, T::Balance> for Module<T> {
	fn transfer(from: T::AccountId, id: T::AssetId, to: T::AccountId, amount: T::Balance) -> sp_std::result::Result<(), &'static str> {
        Self::transfer(from, id, to, amount)
    }
}
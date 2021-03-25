#![cfg_attr(not(feature = "std"), no_std)]

use frame_system::{
	self as system,
	ensure_none,
	offchain::{CreateSignedTransaction, SubmitTransaction},
};
use frame_support::{
	debug,
	dispatch::DispatchResult, decl_module, decl_storage, decl_event,
	traits::Get,
};
use sp_runtime::{
	FixedU128, FixedPointNumber,
	offchain::{http, Duration},
	transaction_validity::{
		InvalidTransaction, ValidTransaction, TransactionValidity, TransactionSource,
		TransactionPriority,
	},
};
use sp_std::vec::Vec;
use lite_json::json::JsonValue;
use pallet_assets as assets;

/// This pallet's configuration trait
pub trait Trait: CreateSignedTransaction<Call<Self>> + assets::Trait {
	/// The overarching event type.
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	/// The overarching dispatch call type.
	type Call: From<Call<Self>>;
	// Traituration parameters
	type UnsignedInterval: Get<Self::BlockNumber>;
	/// A configuration for base priority of unsigned transactions.
	type UnsignedPriority: Get<TransactionPriority>;
}

decl_storage! {
	trait Store for Module<T: Trait> as OffchainWorker {
		/// Defines the block when next unsigned transaction will be accepted.
		NextUnsignedAt get(fn next_unsigned_at): T::BlockNumber;
	}
}

decl_event!(
	/// Events generated by the module.
	pub enum Event<T> where AssetId = <T as assets::Trait>::AssetId {
		NewPrice(AssetId, FixedU128),
	}
);

decl_module! {
	/// A public part of the pallet.
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		fn deposit_event() = default;

		#[weight = 0]
		pub fn submit_price_unsigned(origin, _block_number: T::BlockNumber, price: u32)
			-> DispatchResult
		{
			// This ensures that the function can only be called via unsigned transaction.
			ensure_none(origin)?;
			// Add the price to the on-chain list, but mark it as coming from an empty address.
			let asset_id = <T as assets::Trait>::AssetId::from(4u32);
			let price = FixedU128::saturating_from_rational(price, 100);
			<assets::Module<T>>::_set_price(asset_id, price);
			// now increment the block number at which we expect next unsigned transaction.
			let current_block = <system::Module<T>>::block_number();
			<NextUnsignedAt<T>>::put(current_block + T::UnsignedInterval::get());
			Self::deposit_event(RawEvent::NewPrice(asset_id, price));

			Ok(())
		}

		fn offchain_worker(block_number: T::BlockNumber) {
			debug::native::info!("Hello World from offchain workers!");

			debug::debug!("Current block: {:?}", block_number);

			let price = <assets::Module<T>>::price(<T as assets::Trait>::AssetId::from(4u32));
			debug::debug!("Current price: {:?}", price);

			let res = Self::fetch_price_and_send_raw_unsigned(block_number);
			if let Err(e) = res {
				debug::error!("Error: {}", e);
			}
		}
	}
}

impl<T: Trait> Module<T> {


	/// A helper function to fetch the price and send a raw unsigned transaction.
	fn fetch_price_and_send_raw_unsigned(block_number: T::BlockNumber) -> Result<(), &'static str> {

		let next_unsigned_at = <NextUnsignedAt<T>>::get();
		if next_unsigned_at > block_number {
			return Err("Too early to send unsigned transaction")
		}

		let price = Self::fetch_price().map_err(|_| "Failed to fetch price")?;

		let call = Call::submit_price_unsigned(block_number, price);

		SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into())
			.map_err(|()| "Unable to submit unsigned transaction.")?;

		Ok(())
	}

	/// Fetch current price and return the result in cents.
	fn fetch_price() -> Result<u32, http::Error> {
		let deadline = sp_io::offchain::timestamp().add(Duration::from_millis(2_000));
		let request = http::Request::get(
			"http://localhost:8080/assets/prices"
		);
		let pending = request
			.deadline(deadline)
			.send()
			.map_err(|_| http::Error::IoError)?;

		let response = pending.try_wait(deadline)
			.map_err(|_| http::Error::DeadlineReached)??;
		if response.code != 200 {
			debug::warn!("Unexpected status code: {}", response.code);
			return Err(http::Error::Unknown);
		}

		let body = response.body().collect::<Vec<u8>>();
		debug::warn!("BODY: {:?}", body);

		let body_str = sp_std::str::from_utf8(&body).map_err(|_| {
			debug::warn!("No UTF8 body");
			http::Error::Unknown
		})?;
		debug::warn!("BODY: {}", body_str);
		let price = match Self::parse_price(body_str) {
			Some(price) => Ok(price),
			None => {
				debug::warn!("Unable to extract price from the response: {:?}", body_str);
				Err(http::Error::Unknown)
			}
		}?;

		debug::warn!("Got price: {} cents", price);

		Ok(price)
	}

	/// Parse the price from the given JSON string using `lite-json`.
	///
	/// Returns `None` when parsing failed or `Some(price in cents)` when parsing is successful.
	fn parse_price(price_str: &str) -> Option<u32> {
		let val = lite_json::parse_json(price_str);
		debug::warn!("parsed json: {:?}", val);
		debug::warn!("parsed json raw print: {}", val);
		let price = val.ok().and_then(|v| match v {
			JsonValue::Object(obj) => {
				let mut chars = "USD".chars();
				obj.into_iter()
					.find(|(k, _)| k.iter().all(|k| Some(*k) == chars.next()))
					.and_then(|v| match v.1 {
						JsonValue::Number(number) => Some(number),
						_ => None,
					})
			},
			_ => None
		})?;

		let exp = price.fraction_length.checked_sub(2).unwrap_or(0);
		Some(price.integer as u32 * 100 + (price.fraction / 10_u64.pow(exp)) as u32)
	}

	fn validate_transaction_parameters(
		block_number: &T::BlockNumber,
		new_price: &u32,
	) -> TransactionValidity {
		// Now let's check if the transaction has any chance to succeed.
		let next_unsigned_at = <NextUnsignedAt<T>>::get();
		if &next_unsigned_at > block_number {
			return InvalidTransaction::Stale.into();
		}
		// Let's make sure to reject transactions from the future.
		let current_block = <system::Module<T>>::block_number();
		if &current_block < block_number {
			return InvalidTransaction::Future.into();
		}

		ValidTransaction::with_tag_prefix("OffchainWorker")
			.priority(T::UnsignedPriority::get())
			.and_provides(next_unsigned_at)
			.longevity(5)
			.propagate(true)
			.build()
	}
}

#[allow(deprecated)] // ValidateUnsigned
impl<T: Trait> frame_support::unsigned::ValidateUnsigned for Module<T> {
	type Call = Call<T>;

	fn validate_unsigned(
		_source: TransactionSource,
		call: &Self::Call,
	) -> TransactionValidity {
		// Firstly let's check that we call the right function.
        if let Call::submit_price_unsigned(block_number, new_price) = call {
			Self::validate_transaction_parameters(block_number, new_price)
		} else {
			InvalidTransaction::Call.into()
		}
	}
}

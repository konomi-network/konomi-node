use std::sync::Arc;

use codec::Codec;
use jsonrpc_core::{ErrorCode, Result, Error as RpcError};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic::BlockId,
	traits::Block as BlockT,
};

pub use pallet_lending_rpc_runtime_api::LendingApi as LendingRuntimeApi;

#[rpc]
pub trait LendingApi<BlockHash, AssetId, FixedU128, AccountId, Balance> {

    #[rpc(name = "lending_supply_rate")]
    fn supply_rate(
        &self,
        id: AssetId,
        at: Option<BlockHash>
    ) -> Result<FixedU128>;

    #[rpc(name = "lending_debt_rate")]
    fn debt_rate(
        &self,
        id: AssetId,
        at: Option<BlockHash>
    ) -> Result<FixedU128>;

    #[rpc(name = "lending_get_user_info")]
    fn get_user_info(
        &self,
        user: AccountId,
        at: Option<BlockHash>
    ) -> Result<(Balance, Balance, Balance)>;
}

/// A struct that implements the `SumStorageApi`.
pub struct Lending<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> Lending<C, M> {
    /// Create new `SumStorage` instance with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AssetId, FixedU128, AccountId, Balance> LendingApi<<Block as BlockT>::Hash, AssetId, FixedU128, AccountId, Balance>
    for Lending<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: LendingRuntimeApi<Block, AssetId, FixedU128, AccountId, Balance>,
    AssetId: Codec,
    FixedU128: Codec,
    AccountId: Codec,
	Balance: Codec,
{
    fn supply_rate(
        &self,
        id: AssetId,
        at: Option<<Block as BlockT>::Hash>
    ) -> Result<FixedU128> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash
        ));

        let runtime_api_result = api.supply_rate(&at, id);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876), // No real reason for this value
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn debt_rate(
        &self,
        id: AssetId,
        at: Option<<Block as BlockT>::Hash>
    ) -> Result<FixedU128> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash
        ));

        let runtime_api_result = api.debt_rate(&at, id);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876), // No real reason for this value
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })  
    }

    fn get_user_info(
        &self,
        user: AccountId,
        at: Option<<Block as BlockT>::Hash>
    ) -> Result<(Balance, Balance, Balance)> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash
        ));

        let runtime_api_result = api.get_user_info(&at, user);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876), // No real reason for this value
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })  
    }
}
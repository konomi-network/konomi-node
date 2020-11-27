use std::sync::Arc;

use codec::Codec;
use jsonrpc_core::{ErrorCode, Result, Error as RpcError};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
	generic::BlockId,
	traits::{Block as BlockT, Header as HeaderT},
};

pub use pallet_swap_rpc_runtime_api::SwapApi as SwapRuntimeApi;

#[rpc]
pub trait SwapApi<BlockHash, AssetId, Balance> {
    #[rpc(name = "swap_calculate_output")]
    fn calculate_output(
        &self,
        in_id: AssetId, 
        out_id: AssetId, 
        amount_in: Balance,
        at: Option<BlockHash>
    ) -> Result<Balance>;
}

/// A struct that implements the `SumStorageApi`.
pub struct Swap<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> Swap<C, M> {
    /// Create new `SumStorage` instance with the given reference to the client.
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AssetId, Balance> SwapApi<<Block as BlockT>::Hash, AssetId, Balance>
    for Swap<C, Block>
where
    Block: BlockT,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: SwapRuntimeApi<Block, AssetId, Balance>,
    AssetId: Codec,
	Balance: Codec,
{

    fn calculate_output(
        &self,
        in_id: AssetId, 
        out_id: AssetId, 
        amount_in: Balance,
        at: Option<<Block as BlockT>::Hash>
    ) -> Result<Balance> {

        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(||
            // If the block hash is not supplied assume the best block.
            self.client.info().best_hash
        ));

        let runtime_api_result = api.calculate_output(
            &at, in_id, out_id, amount_in);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876), // No real reason for this value
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
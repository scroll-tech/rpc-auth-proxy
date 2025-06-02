use jsonrpsee::proc_macros::rpc;

use alloy::primitives::{Address, B256, Bytes, U64, U256};
use alloy::rpc::types::BlockId;

use alloy_rpc_types::{BlockNumberOrTag, FeeHistory, Transaction};
use jsonrpsee::core::RpcResult;

// see https://github.com/paradigmxyz/reth/blob/main/crates/rpc/rpc-eth-api/src/core.rs
#[rpc(server, client, namespace = "eth")]
pub trait EthRpcProxy {
    #[method(name = "blockNumber", with_extensions)]
    async fn block_number(&self) -> RpcResult<U256>;

    #[method(name = "chainId", with_extensions)]
    async fn chain_id(&self) -> RpcResult<Option<U64>>;

    #[method(name = "getTransactionByHash", with_extensions)]
    async fn transaction_by_hash(&self, hash: B256) -> RpcResult<Option<Transaction>>;

    #[method(name = "getBalance", with_extensions)]
    async fn balance(&self, address: Address, block_number: Option<BlockId>) -> RpcResult<U256>;

    #[method(name = "getTransactionCount", with_extensions)]
    async fn transaction_count(
        &self,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256>;

    #[method(name = "getCode", with_extensions)]
    async fn get_code(&self, address: Address, block_number: Option<BlockId>) -> RpcResult<Bytes>;

    #[method(name = "gasPrice", with_extensions)]
    async fn gas_price(&self) -> RpcResult<U256>;

    #[method(name = "maxPriorityFeePerGas", with_extensions)]
    async fn max_priority_fee_per_gas(&self) -> RpcResult<U256>;

    #[method(name = "feeHistory", with_extensions)]
    async fn fee_history(
        &self,
        block_count: U64,
        newest_block: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory>;

    #[method(name = "sendRawTransaction", with_extensions)]
    async fn send_raw_transaction(&self, bytes: Bytes) -> RpcResult<B256>;
}

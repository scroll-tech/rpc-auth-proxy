use jsonrpsee::proc_macros::rpc;

use alloy::primitives::{Address, B256, Bytes, U64, U256};
use alloy::rpc::types::BlockId;
use alloy::serde::JsonStorageKey;
use alloy_rpc_types::{
    Block as EthBlock, BlockNumberOrTag, FeeHistory, Filter, Log, TransactionRequest,
};
use jsonrpsee::core::RpcResult;
use scroll_alloy_rpc_types::{ScrollTransactionReceipt as Receipt, Transaction};

pub type Block = EthBlock<Transaction>;

#[rpc(server, client, namespace = "scroll")]
pub trait ScrollRpcProxy {
    #[method(name = "getL1MessagesInBlock", with_extensions)]
    async fn l1_messages_in_block(
        &self,
        block_id: String,
        mode: String,
    ) -> RpcResult<Option<Vec<Transaction>>>;
}

// see https://github.com/paradigmxyz/reth/blob/main/crates/rpc/rpc-eth-api/src/core.rs
#[rpc(server, client, namespace = "eth")]
pub trait EthRpcProxy {
    #[method(name = "blockNumber", with_extensions)]
    async fn block_number(&self) -> RpcResult<U256>;

    #[method(name = "chainId", with_extensions)]
    async fn chain_id(&self) -> RpcResult<Option<U64>>;

    #[method(name = "getBlockByHash", with_extensions)]
    async fn block_by_hash(&self, hash: B256, full: bool) -> RpcResult<Option<Block>>;

    #[method(name = "getBlockByNumber", with_extensions)]
    async fn block_by_number(
        &self,
        number: BlockNumberOrTag,
        full: bool,
    ) -> RpcResult<Option<Block>>;

    #[method(name = "getTransactionByHash", with_extensions)]
    async fn transaction_by_hash(&self, hash: B256) -> RpcResult<Option<Transaction>>;

    #[method(name = "getTransactionReceipt", with_extensions)]
    async fn transaction_receipt(&self, hash: B256) -> RpcResult<Option<Receipt>>;

    #[method(name = "getBalance", with_extensions)]
    async fn balance(&self, address: Address, block_number: Option<BlockId>) -> RpcResult<U256>;

    #[method(name = "getStorageAt", with_extensions)]
    async fn storage_at(
        &self,
        address: Address,
        index: JsonStorageKey,
        block_number: Option<BlockId>,
    ) -> RpcResult<B256>;

    #[method(name = "getTransactionCount", with_extensions)]
    async fn transaction_count(
        &self,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256>;

    #[method(name = "getCode", with_extensions)]
    async fn get_code(&self, address: Address, block_number: Option<BlockId>) -> RpcResult<Bytes>;

    #[method(name = "call", with_extensions)]
    async fn call(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
    ) -> RpcResult<Bytes>;

    #[method(name = "estimateGas", with_extensions)]
    async fn estimate_gas(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256>;

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

    #[method(name = "getLogs", with_extensions)]
    async fn logs(&self, filter: Filter) -> RpcResult<Vec<Log>>;
}

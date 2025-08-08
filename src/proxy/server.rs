use alloy::consensus::transaction::{PooledTransaction, SignerRecoverable};
use alloy::primitives::{Address, B256, Bytes, U64, U256};
use alloy::rpc::types::BlockId;
use alloy_network_primitives::ReceiptResponse;
use alloy_rlp::Decodable;
use alloy_rpc_types::{
    Block, BlockNumberOrTag, FeeHistory, Header, TransactionRequest, TransactionTrait,
};
use hyper::http::Extensions;
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::http_client::HttpClient;
use reth_primitives::TransactionSigned;
use reth_rpc_api::EthApiClient;
use scroll_alloy_rpc_types::{ScrollTransactionReceipt as Receipt, Transaction};

use super::error::{proxy_call_failed, unauthorized};
use super::interface::EthRpcProxyServer;
use crate::auth::AccessLevel;

macro_rules! proxy_call {
    ($client:expr, $method:ident $(, $arg:expr )* ) => {
        EthApiClient::<TransactionRequest, Transaction, Block, Receipt, Header>::$method(&$client $(, $arg )*).await.map_err(|e| proxy_call_failed(e))
    };
}

fn get_access(ext: &Extensions) -> &AccessLevel {
    ext.get::<AccessLevel>().unwrap_or(&AccessLevel::None)
}

fn only_authorized(ext: &Extensions, address: &Address) -> RpcResult<()> {
    if !get_access(ext).is_authorized(address) {
        return Err(unauthorized());
    }
    Ok(())
}

fn only_full_access(ext: &Extensions) -> RpcResult<()> {
    if get_access(ext) != &AccessLevel::Full {
        return Err(unauthorized());
    }
    Ok(())
}

pub struct EthRpcProxyImpl {
    client: HttpClient,
}

impl EthRpcProxyImpl {
    pub fn new(target: impl AsRef<str>) -> anyhow::Result<Self> {
        let client = HttpClient::builder().build(target)?;
        Ok(Self { client })
    }
}

#[async_trait]
impl EthRpcProxyServer for EthRpcProxyImpl {
    async fn block_number(&self, _ext: &Extensions) -> RpcResult<U256> {
        proxy_call!(self.client, block_number)
    }

    async fn chain_id(&self, _ext: &Extensions) -> RpcResult<Option<U64>> {
        proxy_call!(self.client, chain_id)
    }

    async fn balance(
        &self,
        ext: &Extensions,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256> {
        only_authorized(ext, &address)?;
        proxy_call!(self.client, balance, address, block_number)
    }

    async fn transaction_by_hash(
        &self,
        ext: &Extensions,
        hash: B256,
    ) -> RpcResult<Option<Transaction>> {
        let access = get_access(ext);

        // pre-check before proxy call
        if access == &AccessLevel::None {
            return Err(unauthorized());
        }

        // proxy call
        let maybe_tx = proxy_call!(self.client, transaction_by_hash, hash)?;

        let tx = match maybe_tx {
            None => return Ok(None),
            Some(tx) => tx,
        };

        // allow receiver to query transaction
        if access.is_authorized(&tx.to().unwrap_or_default()) {
            return Ok(Some(tx));
        }

        // allow sender to query transaction
        if access.is_authorized(&tx.as_recovered().signer()) {
            return Ok(Some(tx));
        }

        Err(unauthorized())
    }

    async fn transaction_receipt(
        &self,
        ext: &Extensions,
        hash: B256,
    ) -> RpcResult<Option<Receipt>> {
        let access = get_access(ext);

        // pre-check before proxy call
        if access == &AccessLevel::None {
            return Err(unauthorized());
        }

        // proxy call
        let maybe_receipt = proxy_call!(self.client, transaction_receipt, hash)?;

        let receipt = match maybe_receipt {
            None => return Ok(None),
            Some(receipt) => receipt,
        };

        // allow receiver to query transaction
        if access.is_authorized(&receipt.to().unwrap_or_default()) {
            return Ok(Some(receipt));
        }

        // allow sender to query transaction
        if access.is_authorized(&receipt.from()) {
            return Ok(Some(receipt));
        }

        Err(unauthorized())
    }

    async fn transaction_count(
        &self,
        ext: &Extensions,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256> {
        only_authorized(ext, &address)?;
        proxy_call!(self.client, transaction_count, address, block_number)
    }

    async fn get_code(
        &self,
        ext: &Extensions,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<Bytes> {
        only_full_access(ext)?;
        proxy_call!(self.client, get_code, address, block_number)
    }

    async fn call(
        &self,
        ext: &Extensions,
        request: TransactionRequest,
        block_number: Option<BlockId>,
    ) -> RpcResult<Bytes> {
        only_full_access(ext)?;
        proxy_call!(self.client, call, request, block_number, None, None)
    }

    async fn estimate_gas(
        &self,
        ext: &Extensions,
        request: TransactionRequest,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256> {
        only_full_access(ext)?;
        proxy_call!(self.client, estimate_gas, request, block_number, None)
    }

    async fn gas_price(&self, _ext: &Extensions) -> RpcResult<U256> {
        proxy_call!(self.client, gas_price)
    }

    async fn max_priority_fee_per_gas(&self, _ext: &Extensions) -> RpcResult<U256> {
        proxy_call!(self.client, max_priority_fee_per_gas)
    }

    async fn fee_history(
        &self,
        _ext: &Extensions,
        block_count: U64,
        newest_block: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        proxy_call!(
            self.client,
            fee_history,
            block_count,
            newest_block,
            reward_percentiles
        )
    }

    async fn send_raw_transaction(&self, ext: &Extensions, bytes: Bytes) -> RpcResult<B256> {
        let access = get_access(ext);

        // pre-check before tx decoding
        if access == &AccessLevel::None {
            return Err(unauthorized());
        }

        // for basic access, you can only send your own transactions,
        // and contract deployment is not allowed.
        if matches!(access, AccessLevel::Basic(_)) {
            let mut slice: &[u8] = bytes.as_ref();
            let tx: PooledTransaction = TransactionSigned::decode(&mut slice)
                .unwrap()
                .try_into()
                .unwrap();

            let from = tx.recover_signer().unwrap();
            let to = tx.to();
            let selector = tx.function_selector();

            if !access.is_authorized(&from) || to.is_none() {
                return Err(unauthorized());
            }

            if selector.is_some() {
                // check `to` for whitelist
                // check `selector` for whitelist
            }
        }

        proxy_call!(self.client, send_raw_transaction, bytes)
    }
}

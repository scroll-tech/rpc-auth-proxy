use alloy::consensus::transaction::{PooledTransaction, SignerRecoverable};
use alloy::primitives::{Address, B256, Bytes, U64, U256};
use alloy::rpc::types::BlockId;
use alloy::serde::JsonStorageKey;
use alloy_network_primitives::ReceiptResponse;
use alloy_rlp::Decodable;
use alloy_rpc_types::{
    BlockNumberOrTag, FeeHistory, Filter, Log, TransactionRequest, TransactionTrait,
};
use hyper::http::Extensions;
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::http_client::HttpClient;
use reth_primitives::TransactionSigned;
use scroll_alloy_rpc_types::{ScrollTransactionReceipt as Receipt, Transaction};

use super::error::{internal_error, proxy_call_failed, unauthorized};
use super::interface::{
    Block, EthRpcProxyClient, EthRpcProxyServer, ScrollRpcProxyClient, ScrollRpcProxyServer,
    Withdrawal,
};
use crate::auth::AccessLevel;

macro_rules! proxy_call {
    ($client:expr, $method:ident $(, $arg:expr )* ) => {
        EthRpcProxyClient::$method(&$client $(, $arg )*).await.map_err(|e| proxy_call_failed(e))
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

pub struct RpcProxyImpl {
    validium_client: HttpClient,
    withdraw_proofs_client: HttpClient,
}

impl RpcProxyImpl {
    pub fn new(
        validium_url: impl AsRef<str>,
        withdraw_proofs_url: impl AsRef<str>,
    ) -> anyhow::Result<Self> {
        let validium_client = HttpClient::builder().build(validium_url)?;
        let withdraw_proofs_client = HttpClient::builder().build(withdraw_proofs_url)?;
        Ok(Self {
            validium_client,
            withdraw_proofs_client,
        })
    }
}

#[async_trait]
impl ScrollRpcProxyServer for RpcProxyImpl {
    async fn l1_messages_in_block(
        &self,
        ext: &Extensions,
        block_id: String,
        mode: String,
    ) -> RpcResult<Option<Vec<Transaction>>> {
        only_full_access(ext)?;
        ScrollRpcProxyClient::l1_messages_in_block(&self.validium_client, block_id, mode)
            .await
            .map_err(proxy_call_failed)
    }

    async fn withdrawals_by_transaction(
        &self,
        ext: &Extensions,
        tx_hash: B256,
    ) -> RpcResult<Vec<Withdrawal>> {
        let access = get_access(ext);

        // pre-check before proxy call
        if access == &AccessLevel::None {
            return Err(unauthorized());
        }

        // proxy call
        let ws =
            ScrollRpcProxyClient::withdrawals_by_transaction(&self.withdraw_proofs_client, tx_hash)
                .await
                .map_err(proxy_call_failed)?;

        if ws.is_empty() || access == &AccessLevel::Full {
            return Ok(ws);
        }

        // only allow sender to query the transaction
        let maybe_tx = proxy_call!(self.validium_client, transaction_by_hash, tx_hash)?;

        let tx = match maybe_tx {
            // If we found the withdrawal, the transaction must exist
            None => return Err(internal_error("transaction not found")),
            Some(tx) => tx,
        };

        if access.is_authorized(&tx.as_recovered().signer()) {
            return Ok(ws);
        }

        Err(unauthorized())
    }

    async fn withdrawal_by_message_hash(
        &self,
        ext: &Extensions,
        message_hash: B256,
    ) -> RpcResult<Option<Withdrawal>> {
        let access = get_access(ext);

        // pre-check before proxy call
        if access == &AccessLevel::None {
            return Err(unauthorized());
        }

        // proxy call
        let maybe_w = ScrollRpcProxyClient::withdrawal_by_message_hash(
            &self.withdraw_proofs_client,
            message_hash,
        )
        .await
        .map_err(proxy_call_failed)?;

        if maybe_w.is_none() || access == &AccessLevel::Full {
            return Ok(maybe_w);
        }

        let w = maybe_w.expect("checked above");

        // only allow sender to query the transaction
        let maybe_tx = proxy_call!(self.validium_client, transaction_by_hash, w.tx_hash)?;

        let tx = match maybe_tx {
            // If we found the withdrawal, the transaction must exist
            None => return Err(internal_error("transaction not found")),
            Some(tx) => tx,
        };

        if access.is_authorized(&tx.as_recovered().signer()) {
            return Ok(Some(w));
        }

        Err(unauthorized())
    }
}

#[async_trait]
impl EthRpcProxyServer for RpcProxyImpl {
    async fn block_number(&self, _ext: &Extensions) -> RpcResult<U256> {
        proxy_call!(self.validium_client, block_number)
    }

    async fn chain_id(&self, _ext: &Extensions) -> RpcResult<Option<U64>> {
        proxy_call!(self.validium_client, chain_id)
    }

    async fn block_by_hash(
        &self,
        ext: &Extensions,
        hash: B256,
        full: bool,
    ) -> RpcResult<Option<Block>> {
        only_full_access(ext)?;
        proxy_call!(self.validium_client, block_by_hash, hash, full)
    }

    async fn block_by_number(
        &self,
        ext: &Extensions,
        number: BlockNumberOrTag,
        full: bool,
    ) -> RpcResult<Option<Block>> {
        only_full_access(ext)?;
        proxy_call!(self.validium_client, block_by_number, number, full)
    }

    async fn balance(
        &self,
        ext: &Extensions,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256> {
        only_authorized(ext, &address)?;
        proxy_call!(self.validium_client, balance, address, block_number)
    }

    async fn storage_at(
        &self,
        ext: &Extensions,
        address: Address,
        index: JsonStorageKey,
        block_number: Option<BlockId>,
    ) -> RpcResult<B256> {
        only_full_access(ext)?;
        proxy_call!(
            self.validium_client,
            storage_at,
            address,
            index,
            block_number
        )
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
        let maybe_tx = proxy_call!(self.validium_client, transaction_by_hash, hash)?;

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
        let maybe_receipt = proxy_call!(self.validium_client, transaction_receipt, hash)?;

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
        proxy_call!(
            self.validium_client,
            transaction_count,
            address,
            block_number
        )
    }

    async fn get_code(
        &self,
        ext: &Extensions,
        address: Address,
        block_number: Option<BlockId>,
    ) -> RpcResult<Bytes> {
        only_full_access(ext)?;
        proxy_call!(self.validium_client, get_code, address, block_number)
    }

    async fn call(
        &self,
        ext: &Extensions,
        request: TransactionRequest,
        block_number: Option<BlockId>,
    ) -> RpcResult<Bytes> {
        only_full_access(ext)?;
        proxy_call!(self.validium_client, call, request, block_number)
    }

    async fn estimate_gas(
        &self,
        ext: &Extensions,
        request: TransactionRequest,
        block_number: Option<BlockId>,
    ) -> RpcResult<U256> {
        only_full_access(ext)?;
        proxy_call!(self.validium_client, estimate_gas, request, block_number)
    }

    async fn gas_price(&self, _ext: &Extensions) -> RpcResult<U256> {
        Ok(U256::ZERO) // gas is free
    }

    async fn max_priority_fee_per_gas(&self, _ext: &Extensions) -> RpcResult<U256> {
        Ok(U256::ZERO) // gas is free
    }

    async fn fee_history(
        &self,
        _ext: &Extensions,
        block_count: U64,
        newest_block: BlockNumberOrTag,
        reward_percentiles: Option<Vec<f64>>,
    ) -> RpcResult<FeeHistory> {
        proxy_call!(
            self.validium_client,
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

        proxy_call!(self.validium_client, send_raw_transaction, bytes)
    }

    async fn logs(&self, ext: &Extensions, filter: Filter) -> RpcResult<Vec<Log>> {
        only_full_access(ext)?;
        proxy_call!(self.validium_client, logs, filter)
    }
}

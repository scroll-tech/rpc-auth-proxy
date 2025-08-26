use alloy::primitives::TxKind;
use alloy::primitives::{Address, Bytes, FixedBytes};
use alloy::providers::{Provider, ProviderBuilder};
use alloy::sol;
use alloy::sol_types::SolCall;
use alloy_rpc_types::TransactionRequest;
use chrono::Utc;
use jsonrpsee::core::{RpcResult, async_trait};
use jsonrpsee::proc_macros::rpc;
use moka::future::Cache;
use rand::distr::{Alphanumeric, SampleString};
use siwe::{Message, VerificationOpts};
use std::sync::Arc;
use std::time::Duration;

use crate::auth::error::{internal_error, invalid_params};
use crate::auth::jwt::JwtSigner;

type NonceCache = Arc<Cache<String, ()>>;

const NONCE_SIZE: usize = 64;
// ERC-1271 magic value for valid signature
const ERC1271_MAGIC_VALUE: [u8; 4] = [0x16, 0x26, 0xba, 0x7e];
// EIP-7702 code size (0xef0100 + 20-byte address = 23 bytes)
const EIP7702_CODE_SIZE: usize = 23;

// ERC-1271 interface definition
sol! {
    interface IERC1271 {
        function isValidSignature(bytes32 _hash, bytes _signature) external view returns (bytes4 magicValue);
    }
}

/// Account type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccountType {
    /// Externally Owned Account
    Eoa,
    /// Contract Account
    Contract,
    /// EIP-7702 Account (EOA with delegated code)
    Eip7702,
}

#[rpc(server, client, namespace = "siwe")]
pub trait SiweAuthRpc {
    #[method(name = "getNonce")]
    async fn get_nonce(&self) -> RpcResult<String>;

    #[method(name = "signIn")]
    async fn sign_in(&self, message: String, signature: Bytes) -> RpcResult<String>;
}

pub struct SiweAuthRpcImpl {
    cache: NonceCache,
    jwt: JwtSigner,
    // JWT expiration time in seconds, timeout is not exact, there is a 60s leeway by default
    jwt_expiry_secs: usize,
    // RPC provider for get code and signature verification
    rpc_provider: Arc<dyn Provider>,
}

impl SiweAuthRpcImpl {
    pub async fn new(
        jwt: JwtSigner,
        jwt_expiry_secs: usize,
        remote_rpc_url: &str,
    ) -> anyhow::Result<Self> {
        let cache: NonceCache = Arc::new(
            Cache::builder()
                .time_to_live(Duration::from_secs(300))
                .max_capacity(10_000)
                .build(),
        );

        // Create RPC provider for onchain calls
        let rpc_provider = ProviderBuilder::new()
            .connect(remote_rpc_url)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create RPC provider: {}", e))?;

        Ok(Self {
            cache,
            jwt,
            jwt_expiry_secs,
            rpc_provider: Arc::new(rpc_provider),
        })
    }

    /// Classify account type based on code size and content
    async fn classify_account(&self, address: Address) -> anyhow::Result<AccountType> {
        let code = self.rpc_provider.get_code_at(address).await?;

        if code.is_empty() {
            // No code = EOA
            return Ok(AccountType::Eoa);
        }

        if code.len() == EIP7702_CODE_SIZE && code.starts_with(&[0xef, 0x01, 0x00]) {
            // EIP-7702: code size is 23 bytes and starts with 0xef0100
            return Ok(AccountType::Eip7702);
        }

        // Has code but not EIP-7702 = Contract
        Ok(AccountType::Contract)
    }

    /// Verify smart contract signature using ERC-1271
    async fn verify_erc1271_signature(
        &self,
        contract_address: Address,
        message_hash: FixedBytes<32>,
        signature: &Bytes,
    ) -> anyhow::Result<bool> {
        // Construct ERC-1271 call
        let call_data = IERC1271::isValidSignatureCall {
            _hash: message_hash,
            _signature: signature.clone(),
        };

        let request = TransactionRequest {
            to: Some(TxKind::Call(contract_address)),
            input: Some(call_data.abi_encode().into()).into(),
            ..Default::default()
        };

        // Execute onchain call
        let result = self.rpc_provider.call(request).await?;

        // Check if return value matches ERC-1271 magic value
        if result.len() >= 4 {
            let magic_value = &result[0..4];
            Ok(magic_value == ERC1271_MAGIC_VALUE)
        } else {
            Ok(false)
        }
    }

    /// Verify EOA signature using traditional ECDSA
    async fn verify_eoa_signature(
        &self,
        message: &Message,
        signature: &Bytes,
    ) -> anyhow::Result<bool> {
        let opts = VerificationOpts::default();
        match message.verify(signature, &opts).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Unified signature verification function supporting EOA, Contract, and EIP-7702 accounts
    async fn verify_signature(&self, message: &Message, signature: &Bytes) -> anyhow::Result<bool> {
        let address = message.address.into();
        let account_type = self.classify_account(address).await?;

        match account_type {
            AccountType::Eoa => {
                // EOA: use traditional ECDSA verification
                self.verify_eoa_signature(message, signature).await
            }
            AccountType::Contract => {
                // Contract: use ERC-1271 verification
                let message_hash = message.eip191_hash()?;
                self.verify_erc1271_signature(address, message_hash.into(), signature)
                    .await
            }
            AccountType::Eip7702 => {
                // EIP-7702: try both verification methods
                // First try ERC-1271 (delegated code might implement it)
                let message_hash = message.eip191_hash()?;
                let erc1271_result = self
                    .verify_erc1271_signature(address, message_hash.into(), signature)
                    .await;

                match erc1271_result {
                    Ok(true) => {
                        // ERC-1271 verification succeeded
                        Ok(true)
                    }
                    Ok(false) | Err(_) => {
                        // ERC-1271 failed or error, fallback to EOA verification
                        // This handles cases where the delegated code doesn't implement ERC-1271
                        // or the signature is meant for the underlying EOA
                        self.verify_eoa_signature(message, signature).await
                    }
                }
            }
        }
    }
}

#[async_trait]
impl SiweAuthRpcServer for SiweAuthRpcImpl {
    async fn get_nonce(&self) -> RpcResult<String> {
        let nonce = Alphanumeric.sample_string(&mut rand::rng(), NONCE_SIZE);
        self.cache.insert(nonce.clone(), ()).await;
        Ok(nonce)
    }

    async fn sign_in(&self, message: String, signature: Bytes) -> RpcResult<String> {
        // 1. Parse message
        let message = match message.parse::<Message>() {
            Ok(m) => m,
            Err(e) => return Err(invalid_params(format!("invalid message: {e}"))),
        };

        // 2. Verify and consume nonce
        if self.cache.remove(&message.nonce).await.is_none() {
            return Err(invalid_params(format!(
                "invalid message nonce: {}",
                message.nonce
            )));
        }

        // 3. Verify signature (supports EOA, Contract, and EIP-7702)
        match self.verify_signature(&message, &signature).await {
            Ok(true) => {
                // Signature verification successful
            }
            Ok(false) => {
                return Err(invalid_params("invalid message or signature"));
            }
            Err(e) => {
                eprintln!("Signature verification error: {e}");
                return Err(internal_error("signature verification failed"));
            }
        }

        // 4. Issue JWT token
        let exp = (Utc::now().timestamp() as usize) + self.jwt_expiry_secs;
        match self.jwt.create_token(message.address, exp) {
            Ok(token) => Ok(token),
            Err(e) => {
                eprintln!("JWT creation error: {e}");
                Err(internal_error("unable to issue token"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::{JwtSigner, JwtSignerKeyConfig};
    use alloy::primitives::{Address, Bytes};
    use alloy::providers::{Provider, RootProvider, RpcWithBlock};
    use moka::future::Cache;
    use std::collections::HashMap;
    use std::str::FromStr;
    use std::sync::Arc;

    #[derive(Clone)]
    struct MockProvider {
        code_responses: HashMap<Address, Bytes>,
        call_responses: HashMap<Address, Bytes>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                code_responses: HashMap::new(),
                call_responses: HashMap::new(),
            }
        }

        fn with_eoa(mut self, address: Address) -> Self {
            self.code_responses.insert(address, Bytes::new());
            self
        }

        fn with_contract(mut self, address: Address) -> Self {
            self.code_responses
                .insert(address, Bytes::from(vec![0x60, 0x80, 0x60, 0x40]));
            let response = Bytes::from(ERC1271_MAGIC_VALUE.to_vec());
            self.call_responses.insert(address, response);
            self
        }

        fn with_eip7702(mut self, address: Address, delegated_address: Address) -> Self {
            let mut code = vec![0xef, 0x01, 0x00];
            code.extend_from_slice(delegated_address.as_slice());
            self.code_responses.insert(address, Bytes::from(code));
            self.call_responses
                .insert(address, Bytes::from(ERC1271_MAGIC_VALUE.to_vec()));
            self
        }
    }

    impl Provider for MockProvider {
        fn root(&self) -> &RootProvider {
            panic!("MockProvider::root not implemented");
        }

        fn get_code_at(&self, address: Address) -> RpcWithBlock<Address, Bytes> {
            let code_responses = self.code_responses.clone();
            RpcWithBlock::new_provider(move |_| {
                let code_responses = code_responses.clone();
                alloy::providers::ProviderCall::BoxedFuture(Box::pin(async move {
                    Ok(code_responses.get(&address).cloned().unwrap_or_default())
                }))
            })
        }
    }

    fn create_jwt_signer() -> JwtSigner {
        let keys = vec![JwtSignerKeyConfig {
            kid: "test".to_string(),
            secret: "test_secret".to_string(),
        }];
        JwtSigner::from_config(&keys, "test").unwrap()
    }

    fn create_test_impl_with_provider(provider: MockProvider) -> SiweAuthRpcImpl {
        let cache = Arc::new(
            Cache::builder()
                .time_to_live(std::time::Duration::from_secs(300))
                .max_capacity(10_000)
                .build(),
        );
        SiweAuthRpcImpl {
            cache,
            jwt: create_jwt_signer(),
            jwt_expiry_secs: 3600,
            rpc_provider: Arc::new(provider),
        }
    }

    #[tokio::test]
    async fn test_account_classification_with_different_code_patterns() {
        let eoa_addr = Address::from_str("0x1111111111111111111111111111111111111111").unwrap();
        let provider = MockProvider::new().with_eoa(eoa_addr);
        let impl_instance = create_test_impl_with_provider(provider);

        let account_type = impl_instance.classify_account(eoa_addr).await.unwrap();
        assert_eq!(account_type, AccountType::Eoa);

        let contract_addr =
            Address::from_str("0x2222222222222222222222222222222222222222").unwrap();
        let provider = MockProvider::new().with_contract(contract_addr);
        let impl_instance = create_test_impl_with_provider(provider);

        let account_type = impl_instance.classify_account(contract_addr).await.unwrap();
        assert_eq!(account_type, AccountType::Contract);

        let eip7702_addr = Address::from_str("0x3333333333333333333333333333333333333333").unwrap();
        let delegated_addr =
            Address::from_str("0x4444444444444444444444444444444444444444").unwrap();
        let provider = MockProvider::new().with_eip7702(eip7702_addr, delegated_addr);
        let impl_instance = create_test_impl_with_provider(provider);

        let account_type = impl_instance.classify_account(eip7702_addr).await.unwrap();
        assert_eq!(account_type, AccountType::Eip7702);
    }
}

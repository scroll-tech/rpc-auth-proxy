use jsonrpsee::core::ClientError;
use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::types::error::{INTERNAL_ERROR_CODE, INTERNAL_ERROR_MSG};

pub fn unauthorized() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(INTERNAL_ERROR_CODE, "unauthorized", Some("unauthorized"))
}

pub fn proxy_call_failed(e: ClientError) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        INTERNAL_ERROR_CODE,
        INTERNAL_ERROR_MSG,
        Some(format!("proxy_call_failed: {}", e)),
    )
}

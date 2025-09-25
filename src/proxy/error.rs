use jsonrpsee::core::ClientError;
use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::types::error::{INTERNAL_ERROR_CODE, INTERNAL_ERROR_MSG, INVALID_REQUEST_CODE};

pub fn unauthorized() -> ErrorObjectOwned {
    ErrorObjectOwned::owned(INVALID_REQUEST_CODE, "unauthorized", Some("unauthorized"))
}

pub fn internal_error(msg: impl AsRef<str>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(INTERNAL_ERROR_CODE, "internal_error", Some(msg.as_ref()))
}

pub fn proxy_call_failed(e: ClientError) -> ErrorObjectOwned {
    match e {
        jsonrpsee::core::ClientError::Call(e) => e,
        _ => ErrorObjectOwned::owned(
            INTERNAL_ERROR_CODE,
            INTERNAL_ERROR_MSG,
            Some(format!("proxy_call_failed: {e}")),
        ),
    }
}

use jsonrpsee::types::ErrorObjectOwned;
use jsonrpsee::types::error::{
    INTERNAL_ERROR_CODE, INTERNAL_ERROR_MSG, INVALID_PARAMS_CODE, INVALID_PARAMS_MSG,
};

pub fn invalid_params(details: impl AsRef<str>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        INVALID_PARAMS_CODE,
        INVALID_PARAMS_MSG,
        Some(details.as_ref()),
    )
}

pub fn internal_error(details: impl AsRef<str>) -> ErrorObjectOwned {
    ErrorObjectOwned::owned(
        INTERNAL_ERROR_CODE,
        INTERNAL_ERROR_MSG,
        Some(details.as_ref()),
    )
}

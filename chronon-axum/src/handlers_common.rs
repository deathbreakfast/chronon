//! JSON envelope shared by all Chronon API responses.

use serde::{Deserialize, Serialize};

/// Standard API wrapper: `success`, optional `data`, optional `error`.
///
/// Clients should check `success` before reading `data`; HTTP status may still be 200 on logical errors for some routes.
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    /// `true` when `data` is populated and no error occurred.
    pub success: bool,
    /// Payload on success.
    pub data: Option<T>,
    /// Human-readable error when `success` is false.
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    /// Success response with payload.
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    /// Error response with message and no data.
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}

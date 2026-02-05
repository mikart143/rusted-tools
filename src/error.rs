use thiserror::Error;

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Server not found: {0}")]
    ServerNotFound(String),

    #[error("Server already exists: {0}")]
    ServerAlreadyExists(String),

    #[error("Server is not running: {0}")]
    ServerNotRunning(String),

    #[error("Server is already running: {0}")]
    ServerAlreadyRunning(String),

    #[error("Failed to start server: {0}")]
    ServerStartFailed(String),

    #[error("MCP protocol error: {0}")]
    McpProtocol(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Tool not allowed: {0}")]
    ToolNotAllowed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, ProxyError>;

impl ProxyError {
    /// Convert error to HTTP status code
    pub fn status_code(&self) -> axum::http::StatusCode {
        use axum::http::StatusCode;
        match self {
            ProxyError::Config(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ProxyError::ServerNotFound(_) => StatusCode::NOT_FOUND,
            ProxyError::ServerAlreadyExists(_) => StatusCode::CONFLICT,
            ProxyError::ServerNotRunning(_) => StatusCode::SERVICE_UNAVAILABLE,
            ProxyError::ServerAlreadyRunning(_) => StatusCode::CONFLICT,
            ProxyError::ServerStartFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ProxyError::McpProtocol(_) => StatusCode::BAD_GATEWAY,
            ProxyError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ProxyError::Json(_) => StatusCode::BAD_REQUEST,
            ProxyError::InvalidRequest(_) => StatusCode::BAD_REQUEST,
            ProxyError::ToolNotAllowed(_) => StatusCode::FORBIDDEN,
            ProxyError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

// Implement conversion from anyhow::Error for convenience
impl From<anyhow::Error> for ProxyError {
    fn from(err: anyhow::Error) -> Self {
        ProxyError::Internal(err.to_string())
    }
}

impl axum::response::IntoResponse for ProxyError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status_code();
        let body = serde_json::json!({
            "error": self.to_string(),
            "code": status.as_u16(),
        });

        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    #[test]
    fn test_error_status_codes() {
        assert_eq!(
            ProxyError::Config("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            ProxyError::ServerNotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ProxyError::ServerAlreadyExists("test".to_string()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ProxyError::ServerNotRunning("test".to_string()).status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            ProxyError::ServerAlreadyRunning("test".to_string()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            ProxyError::ServerStartFailed("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            ProxyError::ServerStartFailed("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            ProxyError::McpProtocol("test".to_string()).status_code(),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            ProxyError::InvalidRequest("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ProxyError::Internal("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_error_display() {
        let err = ProxyError::ServerNotFound("myserver".to_string());
        assert_eq!(err.to_string(), "Server not found: myserver");
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let proxy_err: ProxyError = io_err.into();
        assert!(matches!(proxy_err, ProxyError::Io(_)));
        assert_eq!(proxy_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_error_from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("{invalid}").unwrap_err();
        let proxy_err: ProxyError = json_err.into();
        assert!(matches!(proxy_err, ProxyError::Json(_)));
        assert_eq!(proxy_err.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_error_from_anyhow() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let proxy_err: ProxyError = anyhow_err.into();
        assert!(matches!(proxy_err, ProxyError::Internal(_)));
        assert!(proxy_err.to_string().contains("something went wrong"));
    }

    #[test]
    fn test_error_into_response() {
        use axum::response::IntoResponse;

        let err = ProxyError::ServerNotFound("test-server".to_string());
        let response = err.into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

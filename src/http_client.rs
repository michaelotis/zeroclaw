//! Shared HTTP client for ZeroClaw.
//!
//! Provides a process-wide [`reqwest::Client`] instance that is reused across
//! tool implementations, provider backends, and channel integrations.  This
//! avoids the overhead of establishing a new TLS session, connection pool,
//! and DNS resolver for every HTTP request.
//!
//! The client is initialised lazily on first access with sensible defaults
//! (120 s request timeout, 10 s connect timeout, gzip, connection pooling)
//! and respects any runtime proxy configuration set via
//! [`crate::config::set_runtime_proxy_config`].
//!
//! Individual call sites that need tighter timeouts can override per-request:
//!
//! ```ignore
//! use crate::http_client::shared_client;
//! shared_client()
//!     .get(url)
//!     .timeout(std::time::Duration::from_secs(5))
//!     .send()
//!     .await?;
//! ```

use std::sync::OnceLock;
use std::time::Duration;

// ── Default shared client ────────────────────────────────────────

static SHARED_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns a shared [`reqwest::Client`] with sensible defaults.
///
/// The client is created once and reused for the lifetime of the process.
///
/// **Defaults:**
/// - 120 s request timeout (override per-request with `.timeout()`)
/// - 10 s connect timeout
/// - gzip decompression enabled
/// - Connection pooling with up to 20 idle connections per host
///
/// **Proxy-aware:** applies runtime proxy settings from `ZEROCLAW_PROXY_*`
/// environment variables.
pub fn shared_client() -> &'static reqwest::Client {
    SHARED_CLIENT.get_or_init(|| {
        let builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .gzip(true)
            .pool_max_idle_per_host(20);
        let builder = crate::config::apply_runtime_proxy_to_builder(builder, "shared");
        builder.build().unwrap_or_else(|e| {
            tracing::warn!("Failed to build shared HTTP client: {e}");
            reqwest::Client::new()
        })
    })
}

// ── No-redirect variant ──────────────────────────────────────────

static SHARED_NO_REDIRECT_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

/// Returns a shared [`reqwest::Client`] that does **not** follow redirects.
///
/// Intended for the HTTP request tool and other security-sensitive call sites
/// where automatic redirect following is undesirable.
///
/// Same defaults as [`shared_client`] except `redirect::Policy::none()`.
pub fn shared_no_redirect_client() -> &'static reqwest::Client {
    SHARED_NO_REDIRECT_CLIENT.get_or_init(|| {
        let builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .gzip(true)
            .pool_max_idle_per_host(20)
            .redirect(reqwest::redirect::Policy::none());
        let builder =
            crate::config::apply_runtime_proxy_to_builder(builder, "shared.no_redirect");
        builder.build().unwrap_or_else(|e| {
            tracing::warn!("Failed to build shared no-redirect HTTP client: {e}");
            reqwest::Client::new()
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_client_returns_same_instance() {
        let a = shared_client() as *const reqwest::Client;
        let b = shared_client() as *const reqwest::Client;
        assert_eq!(a, b, "shared_client() must return the same instance");
    }

    #[test]
    fn shared_no_redirect_client_returns_same_instance() {
        let a = shared_no_redirect_client() as *const reqwest::Client;
        let b = shared_no_redirect_client() as *const reqwest::Client;
        assert_eq!(
            a, b,
            "shared_no_redirect_client() must return the same instance"
        );
    }

    #[test]
    fn shared_clients_are_different_instances() {
        let default = shared_client() as *const reqwest::Client;
        let no_redir = shared_no_redirect_client() as *const reqwest::Client;
        assert_ne!(
            default, no_redir,
            "default and no-redirect clients should be different"
        );
    }
}

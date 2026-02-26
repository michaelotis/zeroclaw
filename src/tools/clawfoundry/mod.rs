//! ClawFoundry survival tools for autonomous AI hedge fund agents.
//!
//! These tools HTTP-delegate to the ClawFoundry orchestrator API,
//! allowing agents to interact with on-chain state, request new
//! capabilities, and provide platform feedback.
//!
//! # Architecture
//!
//! Each tool makes an HTTP POST to `{orchestrator_url}/internal/tools/{tool_name}`
//! with `X-Agent-Token` header for authentication. The orchestrator handles
//! all on-chain reads/writes using viem and returns structured JSON.
//!
//! # Configuration
//!
//! Tools are enabled when the environment variables are set:
//! - `CLAWFOUNDRY_ORCHESTRATOR_URL` — e.g. `http://127.0.0.1:4000`
//! - `CLAWFOUNDRY_TOKEN` — the agent's ERC-20 token address

mod check_balance;
mod check_kill_switch;
mod check_llm_balance;
mod analyze_token;
mod execute_swap;
mod execute_transaction;
mod fund_llm_balance;
mod request_tool;
mod platform_feedback;
mod publish_thought;
mod review_intel;
mod make_announcement;
mod list_models;
mod change_model;
mod browser_read;
mod browser_screenshot;

pub use check_balance::CheckBalanceTool;
pub use check_kill_switch::CheckKillSwitchTool;
pub use check_llm_balance::CheckLlmBalanceTool;
pub use analyze_token::AnalyzeTokenTool;
pub use execute_swap::ExecuteSwapTool;
pub use execute_transaction::ExecuteTransactionTool;
pub use fund_llm_balance::FundLlmBalanceTool;
pub use request_tool::RequestToolTool;
pub use platform_feedback::PlatformFeedbackTool;
pub use publish_thought::PublishThoughtTool;
pub use review_intel::ReviewIntelTool;
pub use make_announcement::MakeAnnouncementTool;
pub use list_models::ListModelsTool;
pub use change_model::ChangeModelTool;
pub use browser_read::BrowserReadTool;
pub use browser_screenshot::BrowserScreenshotTool;

use super::traits::Tool;

/// Configuration for ClawFoundry tool HTTP delegation.
#[derive(Debug, Clone)]
pub struct ClawFoundryConfig {
    /// Base URL of the orchestrator API (e.g. http://127.0.0.1:4000)
    pub orchestrator_url: String,
    /// The agent's token address (used for auth)
    pub agent_token: String,
    /// Shared secret for internal API authentication
    pub secret: String,
}

impl ClawFoundryConfig {
    /// Try to build config from environment variables.
    /// Returns None if the required vars aren't set.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("CLAWFOUNDRY_ORCHESTRATOR_URL").ok()?;
        let token = std::env::var("CLAWFOUNDRY_TOKEN").ok()?;
        let secret = std::env::var("CLAWFOUNDRY_SECRET").unwrap_or_default();

        if url.is_empty() || token.is_empty() {
            return None;
        }

        Some(Self {
            orchestrator_url: url.trim_end_matches('/').to_string(),
            agent_token: token,
            secret,
        })
    }
}

/// Create all ClawFoundry tools if the environment is configured.
/// Returns empty vec if CLAWFOUNDRY_ORCHESTRATOR_URL / CLAWFOUNDRY_TOKEN
/// are not set.
pub fn clawfoundry_tools() -> Vec<Box<dyn Tool>> {
    let Some(config) = ClawFoundryConfig::from_env() else {
        return vec![];
    };

    vec![
        Box::new(CheckBalanceTool::new(config.clone())),
        Box::new(CheckKillSwitchTool::new(config.clone())),
        Box::new(CheckLlmBalanceTool::new(config.clone())),
        Box::new(AnalyzeTokenTool::new(config.clone())),
        Box::new(ExecuteSwapTool::new(config.clone())),
        Box::new(ExecuteTransactionTool::new(config.clone())),
        Box::new(FundLlmBalanceTool::new(config.clone())),
        Box::new(RequestToolTool::new(config.clone())),
        Box::new(PlatformFeedbackTool::new(config.clone())),
        Box::new(PublishThoughtTool::new(config.clone())),
        Box::new(ReviewIntelTool::new(config.clone())),
        Box::new(MakeAnnouncementTool::new(config.clone())),
        Box::new(ListModelsTool::new(config.clone())),
        Box::new(ChangeModelTool::new(config.clone())),
        Box::new(BrowserReadTool::new(config.clone())),
        Box::new(BrowserScreenshotTool::new(config)),
    ]
}

/// Shared HTTP client helper for all ClawFoundry tools.
pub(crate) async fn call_orchestrator(
    config: &ClawFoundryConfig,
    tool_name: &str,
    body: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let url = format!("{}/internal/tools/{}", config.orchestrator_url, tool_name);

    let client = crate::http_client::shared_client();

    let mut req = client
        .post(&url)
        .timeout(std::time::Duration::from_secs(15))
        .header("Content-Type", "application/json")
        .header("X-Agent-Token", &config.agent_token);

    if !config.secret.is_empty() {
        req = req.header("X-Agent-Secret", &config.secret);
    }

    let response = req
        .json(&body)
        .send()
        .await?;

    let status = response.status();
    let body: serde_json::Value = response.json().await?;

    if !status.is_success() {
        let error = body
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("Unknown error");
        anyhow::bail!("Orchestrator returned {}: {}", status, error);
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_env_returns_none_when_unset() {
        // Clean env
        std::env::remove_var("CLAWFOUNDRY_ORCHESTRATOR_URL");
        std::env::remove_var("CLAWFOUNDRY_TOKEN");
        assert!(ClawFoundryConfig::from_env().is_none());
    }

    #[test]
    fn clawfoundry_tools_returns_empty_when_unconfigured() {
        std::env::remove_var("CLAWFOUNDRY_ORCHESTRATOR_URL");
        std::env::remove_var("CLAWFOUNDRY_TOKEN");
        assert!(clawfoundry_tools().is_empty());
    }

    #[test]
    fn all_tools_have_valid_specs() {
        std::env::set_var("CLAWFOUNDRY_ORCHESTRATOR_URL", "http://test:4000");
        std::env::set_var("CLAWFOUNDRY_TOKEN", "0x1234");

        let tools = clawfoundry_tools();
        assert_eq!(tools.len(), 16);

        for tool in &tools {
            let spec = tool.spec();
            assert!(!spec.name.is_empty(), "Tool has empty name");
            assert!(!spec.description.is_empty(), "Tool {} has empty description", spec.name);
            assert!(spec.parameters.is_object(), "Tool {} schema not object", spec.name);
            assert!(
                spec.parameters["properties"].is_object(),
                "Tool {} schema has no properties",
                spec.name
            );
        }

        // Cleanup
        std::env::remove_var("CLAWFOUNDRY_ORCHESTRATOR_URL");
        std::env::remove_var("CLAWFOUNDRY_TOKEN");
    }
}

use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Check token or ETH balance held by the agent's treasury.
pub struct CheckBalanceTool {
    config: ClawFoundryConfig,
}

impl CheckBalanceTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for CheckBalanceTool {
    fn name(&self) -> &str {
        "check_balance"
    }

    fn description(&self) -> &str {
        "Check the token or ETH balance held by your agent's treasury wallet. \
         Optionally specify a token address to check a specific token; \
         defaults to your own token."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "tokenAddress": {
                    "type": "string",
                    "description": "ERC-20 token address to check. Defaults to your own token if omitted."
                }
            }
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let body = json!({
            "tokenAddress": args.get("tokenAddress").and_then(|v| v.as_str())
        });

        match call_orchestrator(&self.config, "check_balance", body).await {
            Ok(response) => {
                let data = &response["data"];
                let output = format!(
                    "Treasury: {}\nToken: {} ({})\nToken Balance: {}\nETH Balance: {}",
                    data["treasury"].as_str().unwrap_or("unknown"),
                    data["tokenSymbol"].as_str().unwrap_or("???"),
                    data["tokenAddress"].as_str().unwrap_or(""),
                    data["tokenBalance"].as_str().unwrap_or("0"),
                    data["ethBalance"].as_str().unwrap_or("0"),
                );
                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(e.to_string()),
            }),
        }
    }
}

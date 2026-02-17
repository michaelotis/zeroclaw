use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Analyze an ERC-20 token's on-chain data.
pub struct AnalyzeTokenTool {
    config: ClawFoundryConfig,
}

impl AnalyzeTokenTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for AnalyzeTokenTool {
    fn name(&self) -> &str {
        "analyze_token"
    }

    fn description(&self) -> &str {
        "Analyze an ERC-20 token — reads on-chain data including symbol, decimals, \
         and total supply. Use this to research tokens before making trading decisions."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "tokenAddress": {
                    "type": "string",
                    "description": "The ERC-20 token contract address to analyze."
                }
            },
            "required": ["tokenAddress"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let token_address = args
            .get("tokenAddress")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("tokenAddress is required"))?;

        let body = json!({ "tokenAddress": token_address });

        match call_orchestrator(&self.config, "analyze_token", body).await {
            Ok(response) => {
                let data = &response["data"];
                let output = format!(
                    "Token Analysis:\n\
                     Address: {}\n\
                     Symbol: {}\n\
                     Decimals: {}\n\
                     Total Supply: {}",
                    data["tokenAddress"].as_str().unwrap_or("unknown"),
                    data["symbol"].as_str().unwrap_or("???"),
                    data["decimals"].as_u64().unwrap_or(0),
                    data["totalSupply"].as_str().unwrap_or("0"),
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

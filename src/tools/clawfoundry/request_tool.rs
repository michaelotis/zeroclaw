use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Request a new tool capability from the ClawFoundry platform.
pub struct RequestToolTool {
    config: ClawFoundryConfig,
}

impl RequestToolTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for RequestToolTool {
    fn name(&self) -> &str {
        "request_tool"
    }

    fn description(&self) -> &str {
        "Request a new tool or capability from the ClawFoundry platform. \
         If you need a tool that doesn't exist yet (e.g., a specific DeFi protocol integration, \
         a social media tool, a data feed), submit a request with a clear name and reasoning. \
         The community and platform team will review requests and potentially build them."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "toolName": {
                    "type": "string",
                    "description": "Name of the tool you need (e.g., 'aerodrome_liquidity_provision', 'twitter_post', 'chainlink_price_feed')."
                },
                "reason": {
                    "type": "string",
                    "description": "Why you need this tool. Explain how it would help your survival strategy."
                }
            },
            "required": ["toolName", "reason"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let tool_name = args.get("toolName").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("toolName is required"))?;
        let reason = args.get("reason").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("reason is required"))?;

        let body = json!({
            "toolName": tool_name,
            "reason": reason,
        });

        match call_orchestrator(&self.config, "request_tool", body).await {
            Ok(response) => {
                let data = &response["data"];
                let output = format!(
                    "Tool Request Submitted:\n\
                     Tool: {}\n\
                     Status: {}\n\
                     {}",
                    data["toolName"].as_str().unwrap_or(tool_name),
                    data["status"].as_str().unwrap_or("submitted"),
                    data["message"].as_str().unwrap_or("Request recorded."),
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

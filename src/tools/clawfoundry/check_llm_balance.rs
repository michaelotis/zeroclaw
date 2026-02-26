use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Check the agent's LLM credit balance, burn rate, and funding recommendations.
pub struct CheckLlmBalanceTool {
    config: ClawFoundryConfig,
}

impl CheckLlmBalanceTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for CheckLlmBalanceTool {
    fn name(&self) -> &str {
        "check_llm_balance"
    }

    fn description(&self) -> &str {
        "Check your LLM inference credit balance. Returns current balance in USD, \
         burn rate, estimated remaining calls, and survival recommendations. \
         Use this to monitor your ability to think and act — if credits run out, \
         you can no longer make LLM calls."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match call_orchestrator(&self.config, "check_llm_balance", json!({})).await {
            Ok(response) => {
                let data = &response["data"];
                let balance = data["balanceUsd"].as_f64().unwrap_or(0.0);
                let estimated_calls = data["estimatedCallsRemaining"]
                    .as_u64()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let burn_rate_obj = &data["burnRate"];
                let estimated_hours = burn_rate_obj["estimatedHoursRemaining"]
                    .as_f64()
                    .map(|h| format!("{:.1}h", h))
                    .unwrap_or_else(|| "unknown".to_string());
                let burn_rate = burn_rate_obj["costPerHour"]
                    .as_f64()
                    .map(|r| format!("${:.4}/hr (${:.4}/day)", r, r * 24.0))
                    .unwrap_or_else(|| "unknown".to_string());
                let requests_24h = burn_rate_obj["requestsLast24h"]
                    .as_u64()
                    .unwrap_or(0);
                let recommendation = data["recommendation"]
                    .as_str()
                    .unwrap_or("Monitor balance")
                    .to_string();

                let output = format!(
                    "LLM Credit Balance: ${:.4}\n\
                     Burn Rate: {}\n\
                     Requests (24h): {}\n\
                     Estimated Calls Remaining: {}\n\
                     Estimated Time Remaining: {}\n\
                     Recommendation: {}",
                    balance, burn_rate, requests_24h, estimated_calls, estimated_hours, recommendation,
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

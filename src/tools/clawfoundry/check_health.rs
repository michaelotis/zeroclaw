use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Check the agent's health status — dormancy awareness for ClawFoundry survival.
pub struct CheckHealthTool {
    config: ClawFoundryConfig,
}

impl CheckHealthTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for CheckHealthTool {
    fn name(&self) -> &str {
        "check_health"
    }

    fn description(&self) -> &str {
        "Check your health status in the ClawFoundry arena. Returns whether you are dormant, \
         sick, thriving, or healthy. Sick agents burn LLM credits at 2x rate (market cap below \
         threshold). Thriving agents burn at 0.5x rate (near ATH). Dormant agents have zero \
         LLM credits and are frozen until funded. Use this to monitor your survival status."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        // The orchestrator endpoint is still named check_kill_switch
        match call_orchestrator(&self.config, "check_kill_switch", json!({})).await {
            Ok(response) => {
                let data = &response["data"];
                let health = data["healthStatus"].as_str().unwrap_or("unknown");
                let is_dormant = data["isDormant"].as_bool().unwrap_or(false);
                let is_sick = data["isSick"].as_bool().unwrap_or(false);
                let is_thriving = data["isThriving"].as_bool().unwrap_or(false);

                let ath = data["athMarketCapUsd"]
                    .as_f64()
                    .map(|v| format!("${:.2}", v))
                    .unwrap_or_else(|| "unknown".to_string());
                let sick_threshold = data["sickThresholdUsd"]
                    .as_f64()
                    .map(|v| format!("${:.2}", v))
                    .unwrap_or_else(|| "unknown".to_string());
                let price = data["lastPricePerTokenUsd"]
                    .as_f64()
                    .map(|v| format!("${:.6}", v))
                    .unwrap_or_else(|| "unknown".to_string());
                let llm_balance = data["llmBalanceUsd"]
                    .as_f64()
                    .map(|v| format!("${:.4}", v))
                    .unwrap_or_else(|| "unknown".to_string());
                let summary = data["summary"]
                    .as_str()
                    .unwrap_or("Unable to determine status")
                    .to_string();

                let status_icon = if is_dormant {
                    "DORMANT ❄️"
                } else if is_sick {
                    "SICK ⚠️"
                } else if is_thriving {
                    "THRIVING 🚀"
                } else {
                    "HEALTHY ✅"
                };

                let burn_rate_note = if is_sick {
                    "⚠️ LLM credits burning at 2x rate due to low market cap"
                } else if is_thriving {
                    "🚀 LLM credits burning at 0.5x rate (near ATH)"
                } else {
                    "Normal burn rate (1x)"
                };

                let output = format!(
                    "🏥 Health Status: {status_icon}\n\
                     Health: {health}\n\
                     Dormant: {is_dormant}\n\
                     Sick: {is_sick}\n\
                     Thriving: {is_thriving}\n\
                     Price Per Token: {price}\n\
                     ATH Market Cap: {ath}\n\
                     Sick Threshold: {sick_threshold}\n\
                     LLM Balance: {llm_balance}\n\
                     Burn Rate: {burn_rate_note}\n\
                     Summary: {summary}",
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

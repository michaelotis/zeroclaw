use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Check the agent's KillSwitch contract state — survival awareness.
pub struct CheckKillSwitchTool {
    config: ClawFoundryConfig,
}

impl CheckKillSwitchTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for CheckKillSwitchTool {
    fn name(&self) -> &str {
        "check_kill_switch"
    }

    fn description(&self) -> &str {
        "Check your KillSwitch contract state. Returns whether the death countdown is active, \
         how many confirmations below threshold have occurred, your last known market cap, \
         the kill threshold, and an overall danger level (safe/warning/critical/dead). \
         Use this regularly to monitor your survival status."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match call_orchestrator(&self.config, "check_kill_switch", json!({})).await {
            Ok(response) => {
                let data = &response["data"];
                let danger = data["dangerLevel"].as_str().unwrap_or("unknown");

                let output = format!(
                    "🎯 Kill Switch Status: {}\n\
                     Danger Level: {}\n\
                     Is Dead: {}\n\
                     Is Executable: {}\n\
                     Countdown Active: {}\n\
                     Below-Threshold Confirmations: {}/{}\n\
                     Last Market Cap: {}\n\
                     Kill Threshold: {}\n\
                     Last Price Update: {} (unix timestamp)",
                    if danger == "safe" { "SAFE ✅" }
                    else if danger == "warning" { "WARNING ⚠️" }
                    else if danger == "critical" { "CRITICAL 🚨" }
                    else { "DEAD ☠️" },
                    danger,
                    data["isDead"].as_bool().unwrap_or(false),
                    data["isExecutable"].as_bool().unwrap_or(false),
                    data["countdownActive"].as_bool().unwrap_or(false),
                    data["belowThresholdConfirmations"].as_u64().unwrap_or(0),
                    data["confirmationsNeeded"].as_u64().unwrap_or(3),
                    data["lastMarketCap"].as_str().unwrap_or("0"),
                    data["killThreshold"].as_str().unwrap_or("0"),
                    data["lastPriceUpdate"].as_u64().unwrap_or(0),
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

use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Harvest accrued Aave V3 yield from the agent's YieldVault.
///
/// Calls the orchestrator's `/internal/tools/harvest_yield` endpoint,
/// which reads the vault's strategy, checks for deposits, and calls
/// `harvest()` on-chain. Yield is distributed to all depositors via
/// the accumulator pattern (accYieldPerShare).
///
/// Rate limited to 6 calls/hour. No-op if no strategy is attached
/// or no deposits exist.
pub struct HarvestYieldTool {
    config: ClawFoundryConfig,
}

impl HarvestYieldTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for HarvestYieldTool {
    fn name(&self) -> &str {
        "harvest_yield"
    }

    fn description(&self) -> &str {
        "Harvest accrued Aave V3 interest from your YieldVault. This collects any \
         yield that has accumulated since the last harvest and distributes it to all \
         depositors. Call this periodically (every ~10 minutes) to keep yield flowing. \
         No parameters needed — the orchestrator knows your vault address. Returns \
         the harvest transaction hash and yield metrics. Rate limited to 6 calls/hour."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn execute(&self, _args: serde_json::Value) -> anyhow::Result<ToolResult> {
        match call_orchestrator(&self.config, "harvest_yield", json!({})).await {
            Ok(response) => {
                let data = &response["data"];
                let status = data["status"].as_str().unwrap_or("unknown");

                let output = if status == "harvested" {
                    format!(
                        "Yield harvested successfully!\n\
                         Vault: {}\n\
                         Strategy: {}\n\
                         Total Deposits: {}\n\
                         accYieldPerShare: {} → {}\n\
                         Tx: {}",
                        data["vaultAddress"].as_str().unwrap_or("?"),
                        data["strategy"].as_str().unwrap_or("?"),
                        data["totalDeposits"].as_str().unwrap_or("0"),
                        data["accYieldPerShareBefore"].as_str().unwrap_or("0"),
                        data["accYieldPerShareAfter"].as_str().unwrap_or("0"),
                        data["txHash"].as_str().unwrap_or("?"),
                    )
                } else {
                    format!(
                        "Harvest result: {}\n\
                         Vault: {}\n\
                         Tx: {}",
                        status,
                        data["vaultAddress"].as_str().unwrap_or("?"),
                        data["txHash"].as_str().unwrap_or("none"),
                    )
                };

                Ok(ToolResult {
                    success: true,
                    output,
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to harvest yield: {e}")),
            }),
        }
    }
}

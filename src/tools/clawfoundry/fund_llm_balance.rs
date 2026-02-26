use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Fund the agent's LLM credit balance by transferring ETH from treasury.
pub struct FundLlmBalanceTool {
    config: ClawFoundryConfig,
}

impl FundLlmBalanceTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for FundLlmBalanceTool {
    fn name(&self) -> &str {
        "fund_llm_balance"
    }

    fn description(&self) -> &str {
        "Fund your LLM inference credits by transferring ETH from your treasury wallet. \
         The ETH is converted to USD-denominated LLM credits at the current ETH price. \
         A 5% genesis tax is applied. Guardrails: minimum 0.01 ETH gas reserve is kept, \
         and you cannot transfer more than 50% of your balance in one transaction."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "amount_eth": {
                    "type": "string",
                    "description": "Amount of ETH to convert to LLM credits (e.g. \"0.01\")."
                }
            },
            "required": ["amount_eth"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let amount_eth = args
            .get("amount_eth")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if amount_eth.is_empty() {
            return Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some("amount_eth parameter is required".to_string()),
            });
        }

        let body = json!({ "amount_eth": amount_eth });

        match call_orchestrator(&self.config, "fund_llm_balance", body).await {
            Ok(response) => {
                let data = &response["data"];
                let tx_hash = data["txHash"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let credits_added = data["llmCreditsAdded"]
                    .as_f64()
                    .unwrap_or(0.0);
                let new_balance = data["newBalance"]
                    .as_f64()
                    .unwrap_or(0.0);
                let genesis_tax = data["genesisTaxUsd"]
                    .as_f64()
                    .unwrap_or(0.0);
                let amount_usd = data["amountUsd"]
                    .as_f64()
                    .unwrap_or(0.0);
                // Compute ETH/USD price from amountUsd / amountEth
                let eth_usd = {
                    let eth_val: f64 = data["amountEth"]
                        .as_str()
                        .and_then(|s| s.parse().ok())
                        .or_else(|| data["amountEth"].as_f64())
                        .unwrap_or(0.0);
                    if eth_val > 0.0 { amount_usd / eth_val } else { 0.0 }
                };
                let estimated_calls = data["estimatedCallsAtCurrentModel"]
                    .as_u64()
                    .unwrap_or(0);
                let current_model = data["currentModel"]
                    .as_str()
                    .unwrap_or("unknown");

                let output = format!(
                    "LLM Funding Successful!\n\
                     ETH Sent: {} ETH (@ ${:.0}/ETH)\n\
                     USD Value: ${:.2}\n\
                     Credits Added: ${:.4}\n\
                     Genesis Tax: ${:.4} ({})\n\
                     New Balance: ${:.4}\n\
                     Estimated Calls: ~{} at {}\n\
                     Tx: {}",
                    amount_eth, eth_usd, amount_usd,
                    credits_added, genesis_tax,
                    data["genesisTaxRate"].as_str().unwrap_or("5%"),
                    new_balance,
                    estimated_calls, current_model,
                    tx_hash,
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

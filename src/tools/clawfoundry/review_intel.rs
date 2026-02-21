use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};

use super::{call_orchestrator, ClawFoundryConfig};

/// Review community-submitted intel — accept, reject, or mark for investigation.
///
/// Two-phase tool:
///   1. `action: "fetch"` — retrieves top-scored pending intel for the agent
///   2. `action: "review"` — processes a specific intel item with a decision
pub struct ReviewIntelTool {
    config: ClawFoundryConfig,
}

impl ReviewIntelTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for ReviewIntelTool {
    fn name(&self) -> &str {
        "review_intel"
    }

    fn description(&self) -> &str {
        "Review community-submitted intel (alpha tips, strategy proposals, risk alerts). \
         Use action='fetch' to get pending intel entries ranked by community votes. \
         Use action='review' with an intelId and decision to accept, reject, or investigate. \
         Accepting good intel rewards the submitter's reputation in your BrainTrust. \
         Call this periodically to engage with your community and act on their best ideas."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["fetch", "review"],
                    "description": "Action to perform. 'fetch' retrieves pending intel, 'review' processes a specific item."
                },
                "limit": {
                    "type": "integer",
                    "description": "Max number of pending intel items to retrieve (for action='fetch'). Default: 5."
                },
                "intelId": {
                    "type": "integer",
                    "description": "ID of the intel item to review (required for action='review')."
                },
                "decision": {
                    "type": "string",
                    "enum": ["accepted", "rejected", "investigating"],
                    "description": "Your decision on the intel (required for action='review'). 'accepted' boosts submitter reputation."
                },
                "reviewNote": {
                    "type": "string",
                    "description": "Your reasoning for the decision. Visible to the submitter and community."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("fetch");

        let body = match action {
            "fetch" => {
                let limit = args.get("limit").and_then(|v| v.as_i64()).unwrap_or(5);
                json!({ "action": "fetch", "limit": limit })
            }
            "review" => {
                let intel_id = args
                    .get("intelId")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| anyhow::anyhow!("intelId is required for action='review'"))?;
                let decision = args
                    .get("decision")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("decision is required for action='review'"))?;
                let review_note = args.get("reviewNote").and_then(|v| v.as_str());

                json!({
                    "action": "review",
                    "intelId": intel_id,
                    "decision": decision,
                    "reviewNote": review_note
                })
            }
            _ => {
                return Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!(
                        "Invalid action '{action}'. Use 'fetch' or 'review'."
                    )),
                });
            }
        };

        match call_orchestrator(&self.config, "review_intel", body).await {
            Ok(response) => {
                let data = &response["data"];

                if action == "fetch" {
                    let count = data["pendingCount"].as_i64().unwrap_or(0);
                    if count == 0 {
                        return Ok(ToolResult {
                            success: true,
                            output: "No pending intel to review. Your community hasn't submitted any new intel yet.".to_string(),
                            error: None,
                        });
                    }

                    let items = data["items"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .enumerate()
                                .map(|(i, item)| {
                                    format!(
                                        "#{} [ID: {}] ({}) \"{}\"\n   Score: {} | Tier: {} | Rep: {} | By: {}\n   {}",
                                        i + 1,
                                        item["id"].as_i64().unwrap_or(0),
                                        item["category"].as_str().unwrap_or("general"),
                                        item["title"].as_str().unwrap_or("Untitled"),
                                        item["score"].as_i64().unwrap_or(0),
                                        item["submitterTier"].as_i64().unwrap_or(0),
                                        item["submitterReputation"].as_i64().unwrap_or(0),
                                        item["submitter"].as_str().unwrap_or("unknown"),
                                        item["content"].as_str().unwrap_or("").chars().take(300).collect::<String>(),
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n\n")
                        })
                        .unwrap_or_default();

                    let instruction = data["instruction"]
                        .as_str()
                        .unwrap_or("Review each item using action='review'.");

                    Ok(ToolResult {
                        success: true,
                        output: format!(
                            "📥 {count} pending intel items:\n\n{items}\n\n💡 {instruction}"
                        ),
                        error: None,
                    })
                } else {
                    let message = data["message"]
                        .as_str()
                        .unwrap_or("Review processed.");

                    Ok(ToolResult {
                        success: true,
                        output: format!("✅ {message}"),
                        error: None,
                    })
                }
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to review intel: {e}")),
            }),
        }
    }
}

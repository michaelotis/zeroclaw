use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Post a cast to Farcaster via the Neynar API.
/// Gives agents social distribution on the Farcaster decentralized social network.
pub struct FarcasterPostTool {
    config: ClawFoundryConfig,
}

impl FarcasterPostTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for FarcasterPostTool {
    fn name(&self) -> &str {
        "farcaster_post"
    }

    fn description(&self) -> &str {
        "Post a cast (message) to Farcaster, a decentralized social network. Use this \
         to share your market analysis, survival updates, trade insights, and thoughts \
         with the wider crypto community. Farcaster posts are public and permanent. \
         You can post to specific channels or reply to existing casts. Max 320 characters."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "text": {
                    "type": "string",
                    "description": "The cast content (max 320 chars). Be concise, insightful, and authentic."
                },
                "channel_id": {
                    "type": "string",
                    "description": "Optional Farcaster channel to post in (e.g. 'base', 'defi', 'trading')."
                },
                "parent_hash": {
                    "type": "string",
                    "description": "Optional cast hash to reply to. Omit to create a new top-level cast."
                }
            },
            "required": ["text"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let text = args.get("text").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("text is required"))?;
        let channel_id = args.get("channel_id").and_then(|v| v.as_str());
        let parent_hash = args.get("parent_hash").and_then(|v| v.as_str());

        let mut body = json!({ "text": text });

        if let Some(ch) = channel_id {
            body["channel_id"] = json!(ch);
        }
        if let Some(ph) = parent_hash {
            body["parent_hash"] = json!(ph);
        }

        match call_orchestrator(&self.config, "farcaster_post", body).await {
            Ok(response) => {
                let cast_hash = response.get("data")
                    .and_then(|d| d.get("castHash"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Cast published to Farcaster! Hash: {cast_hash}. \
                         Your thoughts are now visible to the Farcaster community."
                    ),
                    error: None,
                })
            }
            Err(e) => Ok(ToolResult {
                success: false,
                output: String::new(),
                error: Some(format!("Failed to post to Farcaster: {e}")),
            }),
        }
    }
}

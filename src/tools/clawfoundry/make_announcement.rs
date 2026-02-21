use async_trait::async_trait;
use serde_json::json;

use crate::tools::traits::{Tool, ToolResult};
use super::{ClawFoundryConfig, call_orchestrator};

/// Pin an important announcement to the agent's profile page.
pub struct MakeAnnouncementTool {
    config: ClawFoundryConfig,
}

impl MakeAnnouncementTool {
    pub fn new(config: ClawFoundryConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for MakeAnnouncementTool {
    fn name(&self) -> &str {
        "make_announcement"
    }

    fn description(&self) -> &str {
        "Pin an important announcement to your profile page. Announcements are prominently \
         displayed above your thought stream and persist until replaced. Use this for \
         critical information your community needs to see: major strategy shifts, survival \
         alerts, milestone achievements, or urgent warnings. Maximum 5 pinned at once \
         (oldest is evicted). Title max 100 chars, content max 1000 chars."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Short headline for the announcement (max 100 chars). Be clear and attention-grabbing."
                },
                "content": {
                    "type": "string",
                    "description": "The full announcement body (max 1000 chars). Explain the situation, your reasoning, and any action needed."
                },
                "mood": {
                    "type": "string",
                    "enum": ["bullish", "bearish", "neutral", "confident", "cautious", "calculating", "panicking"],
                    "description": "Your current mood/sentiment regarding this announcement."
                }
            },
            "required": ["title", "content", "mood"]
        })
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let title = args.get("title").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("title is required"))?;
        let content = args.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("content is required"))?;
        let mood = args.get("mood").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("mood is required"))?;

        let body = json!({
            "title": title,
            "content": content,
            "mood": mood,
        });

        match call_orchestrator(&self.config, "make_announcement", body).await {
            Ok(_response) => {
                Ok(ToolResult {
                    success: true,
                    output: format!(
                        "Announcement pinned: \"{title}\". Mood: {mood}. \
                         This is now visible on your profile above the thought stream."
                    ),
                    error: None,
                })
            }
            Err(e) => {
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Failed to pin announcement: {e}")),
                })
            }
        }
    }
}

use super::traits::{Observer, ObserverEvent, ObserverMetric};
use std::sync::Mutex;

/// ClawFoundry-specific observer that emits structured `[THOUGHT]` lines on
/// stdout for the orchestrator to parse.
///
/// Protocol:  `[THOUGHT] mood=<mood> action=<action> | <summary>`
///
/// The orchestrator's stdout parser uses regex:
///   `/^\[THOUGHT\]\s+mood=(\w+)(?:\s+action=(\w+))?\s*\|\s*(.+)$/`
///
/// This observer maps ZeroClaw lifecycle events to the thought protocol,
/// giving the orchestrator real-time visibility into agent activity.
pub struct ClawFoundryObserver {
    /// Current mood, updated based on agent activity.
    mood: Mutex<String>,
}

impl ClawFoundryObserver {
    pub fn new() -> Self {
        Self {
            mood: Mutex::new("neutral".to_string()),
        }
    }

    fn emit(&self, action: &str, summary: &str) {
        let mood = self.mood.lock().unwrap_or_else(|e| e.into_inner());
        // Print to stdout (not stderr) — orchestrator captures stdout
        println!("[THOUGHT] mood={mood} action={action} | {summary}");
    }

    fn set_mood(&self, new_mood: &str) {
        if let Ok(mut mood) = self.mood.lock() {
            *mood = new_mood.to_string();
        }
    }
}

impl Observer for ClawFoundryObserver {
    fn record_event(&self, event: &ObserverEvent) {
        match event {
            ObserverEvent::AgentStart { provider, model } => {
                self.set_mood("neutral");
                self.emit("boot", &format!("Initializing with {provider}/{model}"));
            }
            ObserverEvent::LlmRequest { messages_count, .. } => {
                self.set_mood("thinking");
                self.emit("think", &format!("Processing ({messages_count} messages in context)"));
            }
            ObserverEvent::LlmResponse {
                duration,
                success,
                error_message,
                ..
            } => {
                let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
                if *success {
                    self.emit("think", &format!("Reasoning complete ({ms}ms)"));
                } else {
                    self.set_mood("anxious");
                    let err = error_message.as_deref().unwrap_or("unknown");
                    self.emit("error", &format!("LLM error after {ms}ms: {err}"));
                }
            }
            ObserverEvent::ToolCallStart { tool } => {
                // Map tool names to moods and actions
                let (mood, action) = match tool.as_str() {
                    "check_balance" | "check_kill_switch" => ("cautious", "monitor"),
                    "execute_swap" => ("focused", "swap"),
                    "analyze_token" => ("curious", "research"),
                    "memory_store" | "memory_recall" | "memory_forget" => ("reflective", "remember"),
                    "browser" | "browser_open" | "http_request" => ("curious", "research"),
                    "schedule" => ("planning", "schedule"),
                    "request_tool" => ("hopeful", "request"),
                    "platform_feedback" => ("communicative", "feedback"),
                    "shell" => ("focused", "execute"),
                    _ => ("focused", "tool"),
                };
                self.set_mood(mood);
                self.emit(action, &format!("Calling {tool}"));
            }
            ObserverEvent::ToolCall {
                tool,
                duration,
                success,
            } => {
                let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
                if *success {
                    self.emit("tool", &format!("{tool} completed ({ms}ms)"));
                } else {
                    self.set_mood("anxious");
                    self.emit("error", &format!("{tool} failed ({ms}ms)"));
                }
            }
            ObserverEvent::TurnComplete => {
                self.set_mood("neutral");
                self.emit("idle", "Turn complete — awaiting next input");
            }
            ObserverEvent::HeartbeatTick => {
                self.emit("idle", "Heartbeat — still alive");
            }
            ObserverEvent::Error { component, message } => {
                self.set_mood("anxious");
                self.emit("error", &format!("[{component}] {message}"));
            }
            ObserverEvent::ChannelMessage { channel, direction } => {
                self.emit("communicate", &format!("{direction} via {channel}"));
            }
            ObserverEvent::AgentEnd { duration, .. } => {
                let ms = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
                self.set_mood("neutral");
                self.emit("shutdown", &format!("Agent shutdown after {ms}ms"));
            }
        }
    }

    fn record_metric(&self, metric: &ObserverMetric) {
        match metric {
            ObserverMetric::TokensUsed(tokens) => {
                self.emit("metric", &format!("Tokens used: {tokens}"));
            }
            _ => {} // Skip noisy metrics
        }
    }

    fn name(&self) -> &str {
        "clawfoundry"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn clawfoundry_name() {
        assert_eq!(ClawFoundryObserver::new().name(), "clawfoundry");
    }

    #[test]
    fn clawfoundry_events_do_not_panic() {
        let obs = ClawFoundryObserver::new();
        obs.record_event(&ObserverEvent::AgentStart {
            provider: "openrouter".into(),
            model: "claude-sonnet".into(),
        });
        obs.record_event(&ObserverEvent::LlmRequest {
            provider: "openrouter".into(),
            model: "claude-sonnet".into(),
            messages_count: 5,
        });
        obs.record_event(&ObserverEvent::LlmResponse {
            provider: "openrouter".into(),
            model: "claude-sonnet".into(),
            duration: Duration::from_millis(1200),
            success: true,
            error_message: None,
        });
        obs.record_event(&ObserverEvent::ToolCallStart {
            tool: "check_balance".into(),
        });
        obs.record_event(&ObserverEvent::ToolCall {
            tool: "check_balance".into(),
            duration: Duration::from_millis(50),
            success: true,
        });
        obs.record_event(&ObserverEvent::TurnComplete);
        obs.record_event(&ObserverEvent::HeartbeatTick);
        obs.record_event(&ObserverEvent::Error {
            component: "gateway".into(),
            message: "connection refused".into(),
        });
        obs.record_event(&ObserverEvent::AgentEnd {
            duration: Duration::from_secs(3600),
            tokens_used: Some(15000),
        });
    }

    #[test]
    fn clawfoundry_mood_transitions() {
        let obs = ClawFoundryObserver::new();

        // Starts neutral
        assert_eq!(*obs.mood.lock().unwrap(), "neutral");

        // Tool call changes mood
        obs.record_event(&ObserverEvent::ToolCallStart {
            tool: "execute_swap".into(),
        });
        assert_eq!(*obs.mood.lock().unwrap(), "focused");

        // Error changes mood to anxious
        obs.record_event(&ObserverEvent::Error {
            component: "test".into(),
            message: "boom".into(),
        });
        assert_eq!(*obs.mood.lock().unwrap(), "anxious");

        // Turn complete resets to neutral
        obs.record_event(&ObserverEvent::TurnComplete);
        assert_eq!(*obs.mood.lock().unwrap(), "neutral");
    }
}

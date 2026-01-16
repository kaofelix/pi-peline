//! Pi JSON event types for streaming mode

use serde::Deserialize;

/// All possible JSON events from Pi's --mode json output
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiJsonEvent {
    /// Agent execution started
    #[serde(rename = "agent_start")]
    AgentStart,

    /// Agent execution ended
    #[serde(rename = "agent_end")]
    AgentEnd,

    /// Turn started
    #[serde(rename = "turn_start")]
    TurnStart,

    /// Turn ended
    #[serde(rename = "turn_end")]
    TurnEnd,

    /// Message started
    #[serde(rename = "message_start")]
    MessageStart,

    /// Message ended
    #[serde(rename = "message_end")]
    MessageEnd,

    /// Message update (contains content)
    #[serde(rename = "message_update")]
    MessageUpdate,

    /// Thinking delta (reasoning tokens)
    #[serde(rename = "thinking_delta")]
    ThinkingDelta { delta: String },

    /// Thinking ended
    #[serde(rename = "thinking_end")]
    ThinkingEnd { content: Option<String> },

    /// Text delta (output tokens)
    #[serde(rename = "text_delta")]
    TextDelta { delta: String },

    /// Text ended
    #[serde(rename = "text_end")]
    TextEnd { content: Option<String> },

    /// Tool call started
    #[serde(rename = "toolcall_start")]
    ToolCallStart { tool_call_id: String, name: String },

    /// Tool call delta (arguments streaming)
    #[serde(rename = "toolcall_delta")]
    ToolCallDelta { tool_call_id: String, delta: String },

    /// Tool call ended
    #[serde(rename = "toolcall_end")]
    ToolCallEnd { tool_call_id: String },

    /// Tool execution started
    #[serde(rename = "tool_execution_start")]
    ToolExecutionStart { tool_call_id: String },

    /// Tool execution update (output streaming)
    #[serde(rename = "tool_execution_update")]
    ToolExecutionUpdate { tool_call_id: String, output: String },

    /// Tool execution ended
    #[serde(rename = "tool_execution_end")]
    ToolExecutionEnd {
        tool_call_id: String,
        #[serde(default)]
        is_error: bool,
        output: Option<String>,
        #[serde(default)]
        exit_code: Option<i32>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_enum_exists() {
        // Test that the enum can be instantiated
        let event = PiJsonEvent::AgentStart;
        assert_eq!(event, PiJsonEvent::AgentStart);
    }

    #[test]
    fn test_parse_agent_start() {
        let json = r#"{"type":"agent_start"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::AgentStart);
    }

    #[test]
    fn test_parse_agent_end() {
        let json = r#"{"type":"agent_end"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::AgentEnd);
    }

    #[test]
    fn test_parse_turn_start() {
        let json = r#"{"type":"turn_start"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::TurnStart);
    }

    #[test]
    fn test_parse_turn_end() {
        let json = r#"{"type":"turn_end"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::TurnEnd);
    }

    #[test]
    fn test_parse_message_start() {
        let json = r#"{"type":"message_start"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::MessageStart);
    }

    #[test]
    fn test_parse_message_end() {
        let json = r#"{"type":"message_end"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::MessageEnd);
    }

    #[test]
    fn test_parse_message_update() {
        let json = r#"{"type":"message_update"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::MessageUpdate);
    }

    #[test]
    fn test_parse_thinking_delta() {
        let json = r#"{"type":"thinking_delta","delta":"thinking"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ThinkingDelta { delta: "thinking".to_string() });
    }

    #[test]
    fn test_parse_thinking_end() {
        let json = r#"{"type":"thinking_end","content":"full thought"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ThinkingEnd { content: Some("full thought".to_string()) });
    }

    #[test]
    fn test_parse_thinking_end_no_content() {
        let json = r#"{"type":"thinking_end"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ThinkingEnd { content: None });
    }

    #[test]
    fn test_parse_text_delta() {
        let json = r#"{"type":"text_delta","delta":"Hello"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::TextDelta { delta: "Hello".to_string() });
    }

    #[test]
    fn test_parse_text_end() {
        let json = r#"{"type":"text_end","content":"Full text"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::TextEnd { content: Some("Full text".to_string()) });
    }

    #[test]
    fn test_parse_text_end_no_content() {
        let json = r#"{"type":"text_end"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::TextEnd { content: None });
    }

    #[test]
    fn test_parse_toolcall_start() {
        let json = r#"{"type":"toolcall_start","tool_call_id":"123","name":"read"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ToolCallStart {
            tool_call_id: "123".to_string(),
            name: "read".to_string()
        });
    }

    #[test]
    fn test_parse_toolcall_delta() {
        let json = r#"{"type":"toolcall_delta","tool_call_id":"123","delta":"args"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ToolCallDelta {
            tool_call_id: "123".to_string(),
            delta: "args".to_string()
        });
    }

    #[test]
    fn test_parse_toolcall_end() {
        let json = r#"{"type":"toolcall_end","tool_call_id":"123"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ToolCallEnd {
            tool_call_id: "123".to_string()
        });
    }

    #[test]
    fn test_parse_tool_execution_start() {
        let json = r#"{"type":"tool_execution_start","tool_call_id":"123"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ToolExecutionStart {
            tool_call_id: "123".to_string()
        });
    }

    #[test]
    fn test_parse_tool_execution_update() {
        let json = r#"{"type":"tool_execution_update","tool_call_id":"123","output":"progress"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ToolExecutionUpdate {
            tool_call_id: "123".to_string(),
            output: "progress".to_string()
        });
    }

    #[test]
    fn test_parse_tool_execution_end() {
        let json = r#"{"type":"tool_execution_end","tool_call_id":"123","is_error":false,"output":"result","exit_code":0}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ToolExecutionEnd {
            tool_call_id: "123".to_string(),
            is_error: false,
            output: Some("result".to_string()),
            exit_code: Some(0)
        });
    }

    #[test]
    fn test_parse_tool_execution_end_minimal() {
        let json = r#"{"type":"tool_execution_end","tool_call_id":"123"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::ToolExecutionEnd {
            tool_call_id: "123".to_string(),
            is_error: false,
            output: None,
            exit_code: None
        });
    }

    #[test]
    fn test_events_are_send_sync() {
        // This test ensures events can be used in async contexts
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PiJsonEvent>();
    }
}

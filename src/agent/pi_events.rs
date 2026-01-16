//! Pi JSON event types for streaming mode

use serde::Deserialize;
use serde_json::Value;

/// All possible JSON events from Pi's --mode json output
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiJsonEvent {
    /// Agent execution started
    AgentStart,

    /// Agent execution ended
    AgentEnd,

    /// Turn started
    TurnStart,

    /// Turn ended
    #[serde(rename_all = "camelCase")]
    TurnEnd {
        message: Option<Message>,
        tool_results: Vec<Value>,
    },

    /// Message started
    MessageStart,

    /// Message ended
    MessageEnd,

    /// Message update (contains nested assistant events)
    MessageUpdate {
        /// Nested assistant event (contains type, delta, etc.)
        #[serde(rename = "assistantMessageEvent", default)]
        assistant_message_event: Option<AssistantMessageEvent>,

        /// Full message object
        #[serde(default)]
        message: Option<Message>,
    },

    /// Session start (pi CLI session metadata)
    Session {
        version: u32,
        id: String,
        timestamp: String,
        cwd: String,
    },

    /// Tool execution started
    #[serde(rename_all = "camelCase")]
    ToolExecutionStart {
        tool_call_id: String,
        tool_name: String,
        args: Value,
    },

    /// Tool execution update (partial result)
    #[serde(rename_all = "camelCase")]
    ToolExecutionUpdate {
        tool_call_id: String,
        tool_name: String,
        args: Value,
        partial_result: Value,
    },

    /// Tool execution ended
    #[serde(rename_all = "camelCase")]
    ToolExecutionEnd {
        tool_call_id: String,
        tool_name: String,
        result: Value,
        is_error: bool,
    },
}

/// Nested assistant message event from MessageUpdate
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantMessageEvent {
    /// Thinking started
    #[serde(rename_all = "camelCase")]
    ThinkingStart {
        content_index: usize,
        partial: Message,
    },

    /// Thinking delta (reasoning tokens)
    #[serde(rename_all = "camelCase")]
    ThinkingDelta {
        content_index: usize,
        delta: String,
    },

    /// Thinking ended
    #[serde(rename_all = "camelCase")]
    ThinkingEnd {
        content_index: usize,
        content: Option<String>,
    },

    /// Text started
    #[serde(rename_all = "camelCase")]
    TextStart {
        content_index: usize,
        partial: Message,
    },

    /// Text delta (output tokens)
    #[serde(rename_all = "camelCase")]
    TextDelta {
        content_index: usize,
        delta: String,
    },

    /// Text ended
    #[serde(rename_all = "camelCase")]
    TextEnd {
        content_index: usize,
        content: Option<String>,
    },

    /// Tool call started
    #[serde(rename_all = "camelCase")]
    ToolcallStart {
        content_index: usize,
        partial: Message,
    },

    /// Tool call delta (arguments streaming in)
    #[serde(rename_all = "camelCase")]
    ToolcallDelta {
        content_index: usize,
        delta: String,
        partial: Message,
    },

    /// Tool call ended
    #[serde(rename_all = "camelCase")]
    ToolcallEnd {
        content_index: usize,
        tool_call: ToolCall,
        partial: Message,
    },

    /// Other message events
    #[serde(other)]
    Other,
}

/// Message object
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: Vec<Value>,
}

/// Tool call object
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ToolCall {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_parse_session() {
        let json = r#"{"type":"session","version":3,"id":"abc","timestamp":"2024-01-01T00:00:00Z","cwd":"/test"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        match event {
            PiJsonEvent::Session { version, id, timestamp: _, cwd } => {
                assert_eq!(version, 3);
                assert_eq!(id, "abc");
                assert_eq!(cwd, "/test");
            }
            _ => panic!("Expected Session event"),
        }
    }

    #[test]
    fn test_parse_message_update_with_text_delta() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":1,"delta":"Hello"}}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                assert!(assistant_message_event.is_some());
                match assistant_message_event.unwrap() {
                    AssistantMessageEvent::TextDelta { delta, .. } => {
                        assert_eq!(delta, "Hello");
                    }
                    _ => panic!("Expected TextDelta"),
                }
            }
            _ => panic!("Expected MessageUpdate"),
        }
    }

    #[test]
    fn test_parse_message_update_with_text_end() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"text_end","contentIndex":1,"content":"Full text"}}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                assert!(assistant_message_event.is_some());
                match assistant_message_event.unwrap() {
                    AssistantMessageEvent::TextEnd { content, .. } => {
                        assert_eq!(content, Some("Full text".to_string()));
                    }
                    _ => panic!("Expected TextEnd"),
                }
            }
            _ => panic!("Expected MessageUpdate"),
        }
    }

    #[test]
    fn test_parse_message_update_with_thinking_delta() {
        let json = r#"{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":"Thinking"}}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                assert!(assistant_message_event.is_some());
                match assistant_message_event.unwrap() {
                    AssistantMessageEvent::ThinkingDelta { delta, .. } => {
                        assert_eq!(delta, "Thinking");
                    }
                    _ => panic!("Expected ThinkingDelta"),
                }
            }
            _ => panic!("Expected MessageUpdate"),
        }
    }

    #[test]
    fn test_events_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PiJsonEvent>();
    }

    /// Test: thinking_start event has contentIndex and partial message
    #[test]
    fn test_parse_thinking_start_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "thinking_start",
            "contentIndex": 0,
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "thinking",
                "thinking": "The user",
                "thinkingSignature": "reasoning_content"
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "thinking",
              "thinking": "The user",
              "thinkingSignature": "reasoning_content"
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, message } => {
                // Verify outer MessageUpdate parsed correctly
                assert!(message.is_some());
                assert_eq!(message.unwrap().role, "assistant");

                // Verify the nested AssistantMessageEvent is ThinkingStart
                match assistant_message_event {
                    Some(AssistantMessageEvent::ThinkingStart {
                        content_index,
                        partial,
                    }) => {
                        assert_eq!(content_index, 0);
                        assert_eq!(partial.role, "assistant");
                    }
                    other => panic!("Expected ThinkingStart, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: thinking_delta event has contentIndex, delta, and partial
    #[test]
    fn test_parse_thinking_delta_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "thinking_delta",
            "contentIndex": 0,
            "delta": " just said",
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "thinking",
                "thinking": "The user just said",
                "thinkingSignature": "reasoning_content"
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "thinking",
              "thinking": "The user just said",
              "thinkingSignature": "reasoning_content"
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                match assistant_message_event {
                    Some(AssistantMessageEvent::ThinkingDelta {
                        content_index,
                        delta,
                        ..
                    }) => {
                        assert_eq!(content_index, 0);
                        assert_eq!(delta, " just said");
                    }
                    other => panic!("Expected ThinkingDelta, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: text_start event has contentIndex and partial message
    #[test]
    fn test_parse_text_start_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "text_start",
            "contentIndex": 1,
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "thinking",
                "thinking": "The user said hi",
                "thinkingSignature": "reasoning_content"
              }, {
                "type": "text",
                "text": "Hello"
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "thinking",
              "thinking": "The user said hi",
              "thinkingSignature": "reasoning_content"
            }, {
              "type": "text",
              "text": "Hello"
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                match assistant_message_event {
                    Some(AssistantMessageEvent::TextStart {
                        content_index,
                        ..
                    }) => {
                        assert_eq!(content_index, 1);
                    }
                    other => panic!("Expected TextStart, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: thinking_end event has contentIndex and content string
    #[test]
    fn test_parse_thinking_end_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "thinking_end",
            "contentIndex": 0,
            "content": "The user said hi",
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "thinking",
                "thinking": "The user said hi",
                "thinkingSignature": "reasoning_content"
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "thinking",
              "thinking": "The user said hi",
              "thinkingSignature": "reasoning_content"
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                match assistant_message_event {
                    Some(AssistantMessageEvent::ThinkingEnd {
                        content_index,
                        content,
                        ..
                    }) => {
                        assert_eq!(content_index, 0);
                        assert_eq!(content, Some("The user said hi".to_string()));
                    }
                    other => panic!("Expected ThinkingEnd, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: text_end event has contentIndex and content string
    #[test]
    fn test_parse_text_end_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "text_end",
            "contentIndex": 1,
            "content": "Hello world",
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "thinking",
                "thinking": "The user said hi",
                "thinkingSignature": "reasoning_content"
              }, {
                "type": "text",
                "text": "Hello world"
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "thinking",
              "thinking": "The user said hi",
              "thinkingSignature": "reasoning_content"
            }, {
              "type": "text",
              "text": "Hello world"
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                match assistant_message_event {
                    Some(AssistantMessageEvent::TextEnd {
                        content_index,
                        content,
                        ..
                    }) => {
                        assert_eq!(content_index, 1);
                        assert_eq!(content, Some("Hello world".to_string()));
                    }
                    other => panic!("Expected TextEnd, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: toolcall_start event has contentIndex and partial message with toolCall
    #[test]
    fn test_parse_toolcall_start_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "toolcall_start",
            "contentIndex": 1,
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "thinking",
                "thinking": "Need to list files",
                "thinkingSignature": "reasoning_content"
              }, {
                "type": "toolCall",
                "id": "call_test123",
                "name": "bash",
                "arguments": {
                  "command": "ls"
                }
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "thinking",
              "thinking": "Need to list files",
              "thinkingSignature": "reasoning_content"
            }, {
              "type": "toolCall",
              "id": "call_test123",
              "name": "bash",
              "arguments": {
                "command": "ls"
              }
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                match assistant_message_event {
                    Some(AssistantMessageEvent::ToolcallStart {
                        content_index,
                        partial,
                    }) => {
                        assert_eq!(content_index, 1);
                        assert_eq!(partial.role, "assistant");
                    }
                    other => panic!("Expected ToolcallStart, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: toolcall_delta event has contentIndex, delta, and partial
    #[test]
    fn test_parse_toolcall_delta_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "toolcall_delta",
            "contentIndex": 1,
            "delta": "{\"command\":\"ls -la\"}",
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "toolCall",
                "id": "call_test123",
                "name": "bash",
                "arguments": {
                  "command": "ls -la"
                }
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "toolCall",
              "id": "call_test123",
              "name": "bash",
              "arguments": {
                "command": "ls -la"
              }
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                match assistant_message_event {
                    Some(AssistantMessageEvent::ToolcallDelta {
                        content_index,
                        delta,
                        ..
                    }) => {
                        assert_eq!(content_index, 1);
                        assert_eq!(delta, r#"{"command":"ls -la"}"#);
                    }
                    other => panic!("Expected ToolcallDelta, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: toolcall_end event has contentIndex, toolCall, and partial
    #[test]
    fn test_parse_toolcall_end_event() {
        let json = r#"{
          "type": "message_update",
          "assistantMessageEvent": {
            "type": "toolcall_end",
            "contentIndex": 1,
            "toolCall": {
              "type": "toolCall",
              "id": "call_test123",
              "name": "bash",
              "arguments": {
                "command": "ls -la"
              }
            },
            "partial": {
              "role": "assistant",
              "content": [{
                "type": "toolCall",
                "id": "call_test123",
                "name": "bash",
                "arguments": {
                  "command": "ls -la"
                }
              }],
              "api": "openai-completions",
              "provider": "zai",
              "model": "glm-4.7",
              "usage": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "totalTokens": 0,
                "cost": {
                  "input": 0,
                  "output": 0,
                  "cacheRead": 0,
                  "cacheWrite": 0,
                  "total": 0
                }
              },
              "stopReason": "stop",
              "timestamp": 1768585654198
            }
          },
          "message": {
            "role": "assistant",
            "content": [{
              "type": "toolCall",
              "id": "call_test123",
              "name": "bash",
              "arguments": {
                "command": "ls -la"
              }
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 0,
              "output": 0,
              "cacheRead": 0,
              "cacheWrite": 0,
              "totalTokens": 0,
              "cost": {
                "input": 0,
                "output": 0,
                "cacheRead": 0,
                "cacheWrite": 0,
                "total": 0
              }
            },
            "stopReason": "stop",
            "timestamp": 1768585654198
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::MessageUpdate { assistant_message_event, .. } => {
                match assistant_message_event {
                    Some(AssistantMessageEvent::ToolcallEnd {
                        content_index,
                        tool_call,
                        partial,
                    }) => {
                        assert_eq!(content_index, 1);
                        assert_eq!(tool_call.name, "bash");
                        assert_eq!(tool_call.id, "call_test123");
                    }
                    other => panic!("Expected ToolcallEnd, got: {:?}", other),
                }
            }
            _ => panic!("Expected MessageUpdate, got: {:?}", event),
        }
    }

    /// Test: tool_execution_start event has toolCallId, toolName, and args
    #[test]
    fn test_parse_tool_execution_start_event() {
        let json = r#"{
          "type": "tool_execution_start",
          "toolCallId": "call_test123",
          "toolName": "bash",
          "args": {
            "command": "ls -la"
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::ToolExecutionStart {
                tool_call_id,
                tool_name,
                args,
            } => {
                assert_eq!(tool_call_id, "call_test123");
                assert_eq!(tool_name, "bash");
            }
            _ => panic!("Expected ToolExecutionStart, got: {:?}", event),
        }
    }

    /// Test: tool_execution_update event has toolCallId, toolName, args, and partialResult
    #[test]
    fn test_parse_tool_execution_update_event() {
        let json = r#"{
          "type": "tool_execution_update",
          "toolCallId": "call_test123",
          "toolName": "bash",
          "args": {
            "command": "ls -la"
          },
          "partialResult": {
            "content": [{
              "type": "text",
              "text": "total 280\ndrwxr-xr-x"
            }]
          }
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::ToolExecutionUpdate {
                tool_call_id,
                tool_name,
                ..
            } => {
                assert_eq!(tool_call_id, "call_test123");
                assert_eq!(tool_name, "bash");
            }
            _ => panic!("Expected ToolExecutionUpdate, got: {:?}", event),
        }
    }

    /// Test: tool_execution_end event has toolCallId, toolName, result, and isError
    #[test]
    fn test_parse_tool_execution_end_event() {
        let json = r#"{
          "type": "tool_execution_end",
          "toolCallId": "call_test123",
          "toolName": "bash",
          "result": {
            "content": [{
              "type": "text",
              "text": "total 280\ndrwxr-xr-x   21 kaofelix  staff    672 Jan 16 18:28 .\n"
            }]
          },
          "isError": false
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::ToolExecutionEnd {
                tool_call_id,
                tool_name,
                is_error,
                ..
            } => {
                assert_eq!(tool_call_id, "call_test123");
                assert_eq!(tool_name, "bash");
                assert_eq!(is_error, false);
            }
            _ => panic!("Expected ToolExecutionEnd, got: {:?}", event),
        }
    }

    /// Test: turn_start event (simple no-payload event)
    #[test]
    fn test_parse_turn_start_event() {
        let json = r#"{"type":"turn_start"}"#;
        let event: PiJsonEvent = serde_json::from_str(json).unwrap();
        assert_eq!(event, PiJsonEvent::TurnStart);
    }

    /// Test: turn_end event has message and toolResults
    #[test]
    fn test_parse_turn_end_event() {
        let json = r#"{
          "type": "turn_end",
          "message": {
            "role": "assistant",
            "content": [{
              "type": "toolCall",
              "id": "call_test123",
              "name": "bash",
              "arguments": {
                "command": "ls -la"
              }
            }],
            "api": "openai-completions",
            "provider": "zai",
            "model": "glm-4.7",
            "usage": {
              "input": 29,
              "output": 67,
              "cacheRead": 1575,
              "cacheWrite": 0,
              "totalTokens": 1671,
              "cost": {
                "input": 0.0000174,
                "output": 0.0001474,
                "cacheRead": 0.00017325,
                "cacheWrite": 0,
                "total": 0.00033805
              }
            },
            "stopReason": "toolUse",
            "timestamp": 1768587010813
          },
          "toolResults": [{
            "role": "toolResult",
            "toolCallId": "call_test123",
            "toolName": "bash",
            "content": [{
              "type": "text",
              "text": "total 280\ndrwxr-xr-x"
            }],
            "isError": false,
            "timestamp": 1768587010811
          }]
        }"#;

        let event: PiJsonEvent = serde_json::from_str(json).unwrap();

        match event {
            PiJsonEvent::TurnEnd { message, tool_results } => {
                assert!(message.is_some());
                assert_eq!(message.unwrap().role, "assistant");
                assert_eq!(tool_results.len(), 1);
            }
            _ => panic!("Expected TurnEnd, got: {:?}", event),
        }
    }
}

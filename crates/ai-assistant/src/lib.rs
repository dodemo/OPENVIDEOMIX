//! AI Assistant & LLM Integration for OPENVIDEOMIX
//!
//! Handles communication with local models on "PCgrande" (Ollama/vLLM)
//! and OpenAI / Codex API endpoints for real-time broadcast automation.

use openvideomix_core::InputId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiBackend {
    LocalPCGrande { endpoint_url: String, model_name: String },
    OpenAiEnterprise { api_key: String, model: String },
    Disabled,
}

impl Default for AiBackend {
    fn default() -> Self {
        Self::LocalPCGrande {
            endpoint_url: "http://127.0.0.1:11434/v1".into(),
            model_name: "qwen2.5-coder:latest".into(),
        }
    }
}

/// Broadcast suggestion or action triggered by AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AiBroadcastAction {
    SwitchCamera { target_input: InputId, reason: String },
    DisplayLowerThird { text: String, subtext: String, duration_secs: u32 },
    MuteChannel { input_id: InputId },
    Warning { message: String },
}

pub struct AiAssistantEngine {
    pub backend: AiBackend,
    pub is_enabled: bool,
    pub auto_pilot_enabled: bool,
}

impl Default for AiAssistantEngine {
    fn default() -> Self {
        Self {
            backend: AiBackend::default(),
            is_enabled: true,
            auto_pilot_enabled: false,
        }
    }
}

impl AiAssistantEngine {
    pub fn new(backend: AiBackend) -> Self {
        Self {
            backend,
            is_enabled: true,
            auto_pilot_enabled: false,
        }
    }

    /// Parse natural language voice/chat commands from the director
    pub fn parse_director_command(&self, prompt: &str) -> Option<AiBroadcastAction> {
        let lower = prompt.to_lowercase();
        if lower.contains("cámara 1") || lower.contains("cam 1") || lower.contains("camera 1") {
            Some(AiBroadcastAction::SwitchCamera {
                target_input: 1,
                reason: "Orde do director por voz".into(),
            })
        } else if lower.contains("cámara 2") || lower.contains("cam 2") || lower.contains("camera 2") {
            Some(AiBroadcastAction::SwitchCamera {
                target_input: 2,
                reason: "Orde do director por voz".into(),
            })
        } else if lower.contains("rótulo") || lower.contains("lower third") || lower.contains("titular") {
            Some(AiBroadcastAction::DisplayLowerThird {
                text: "Emisión en Directo".into(),
                subtext: "OPENVIDEOMIX".into(),
                duration_secs: 5,
            })
        } else {
            None
        }
    }
}

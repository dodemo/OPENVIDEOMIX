//! Minimal OpenAction plugin for the OPENVIDEOMIX localhost control API.
//! Works only when the host plugin process receives the same bearer token as
//! OPENVIDEOMIX. Do not store tokens in exported OpenDeck profile settings.

use openaction::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

const API: &str = "http://127.0.0.1:21873/v1/command";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct EmptySettings {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InputSettings {
    pub input: u32,
}

impl Default for InputSettings {
    fn default() -> Self {
        Self { input: 1 }
    }
}

pub fn validate_token(token: &str) -> bool {
    token.len() >= 32 && token.len() <= 256 && token.is_ascii()
}

pub fn make_cut_payload() -> Value {
    json!({"action": "cut"})
}

pub fn make_preview_payload(input: u32) -> Value {
    json!({
        "action": "select_preview",
        "args": { "input": input }
    })
}

pub fn make_program_payload(input: u32) -> Value {
    json!({
        "action": "select_program",
        "args": { "input": input }
    })
}

pub fn make_overlay1_payload(input: u32) -> Value {
    json!({
        "action": "toggle_overlay1",
        "args": { "input": input }
    })
}

async fn dispatch(payload: Value) -> bool {
    let Ok(token) = std::env::var("OPENVIDEOMIX_CONTROL_TOKEN") else {
        return false;
    };
    if !validate_token(&token) {
        return false;
    }
    let client = reqwest::Client::new();
    match client
        .post(API)
        .bearer_auth(token)
        .timeout(Duration::from_secs(4))
        .json(&payload)
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

struct CutAction;

#[async_trait]
impl Action for CutAction {
    const UUID: ActionUuid = "com.openvideomix.control.cut";
    type Settings = EmptySettings;

    async fn key_up(
        &self,
        instance: &Instance,
        _settings: &Self::Settings,
    ) -> OpenActionResult<()> {
        let ok = dispatch(make_cut_payload()).await;
        instance
            .set_title(Some(if ok { "CUT" } else { "OFFLINE" }.to_string()), None)
            .await
    }
}

struct PreviewAction;

#[async_trait]
impl Action for PreviewAction {
    const UUID: ActionUuid = "com.openvideomix.control.preview";
    type Settings = InputSettings;

    async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
        let ok = settings.input > 0 && dispatch(make_preview_payload(settings.input)).await;
        let label = if ok {
            format!("PVW {}", settings.input)
        } else {
            "OFFLINE".to_string()
        };
        instance.set_title(Some(label), None).await
    }
}

struct ProgramAction;

#[async_trait]
impl Action for ProgramAction {
    const UUID: ActionUuid = "com.openvideomix.control.program";
    type Settings = InputSettings;

    async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
        let ok = settings.input > 0 && dispatch(make_program_payload(settings.input)).await;
        let label = if ok {
            format!("PGM {}", settings.input)
        } else {
            "OFFLINE".to_string()
        };
        instance.set_title(Some(label), None).await
    }
}

struct OverlayAction;

#[async_trait]
impl Action for OverlayAction {
    const UUID: ActionUuid = "com.openvideomix.control.overlay1";
    type Settings = InputSettings;

    async fn key_up(&self, instance: &Instance, settings: &Self::Settings) -> OpenActionResult<()> {
        let ok = settings.input > 0 && dispatch(make_overlay1_payload(settings.input)).await;
        let label = if ok {
            format!("OV1 {}", settings.input)
        } else {
            "OFFLINE".to_string()
        };
        instance.set_title(Some(label), None).await
    }
}

#[tokio::main]
async fn main() -> OpenActionResult<()> {
    register_action(CutAction).await;
    register_action(PreviewAction).await;
    register_action(ProgramAction).await;
    register_action(OverlayAction).await;
    run(std::env::args().collect()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_token_length_and_charset() {
        assert!(!validate_token(""));
        assert!(!validate_token("short_token"));
        assert!(!validate_token(&"a".repeat(31)));
        assert!(validate_token(&"a".repeat(32)));
        assert!(validate_token(&"a".repeat(256)));
        assert!(!validate_token(&"a".repeat(257)));
        assert!(!validate_token(&format!("{}ñ", "a".repeat(31))));
    }

    #[test]
    fn serializes_expected_action_payloads() {
        assert_eq!(make_cut_payload(), json!({"action": "cut"}));
        assert_eq!(
            make_preview_payload(3),
            json!({"action": "select_preview", "args": {"input": 3}})
        );
        assert_eq!(
            make_program_payload(2),
            json!({"action": "select_program", "args": {"input": 2}})
        );
        assert_eq!(
            make_overlay1_payload(1),
            json!({"action": "toggle_overlay1", "args": {"input": 1}})
        );
    }

    #[test]
    fn input_settings_deserializes_default_and_custom() {
        let default_settings: InputSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(default_settings.input, 1);

        let custom_settings: InputSettings = serde_json::from_str(r#"{"input": 4}"#).unwrap();
        assert_eq!(custom_settings.input, 4);
    }
}

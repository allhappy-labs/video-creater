use serde::{Deserialize, Serialize};

pub const COMPATIBILITY_PROTOCOL_NAME: &str = "video-creater.compatibility";
pub const COMPATIBILITY_PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityRequest {
    pub protocol: String,
    pub schema_version: u32,
    pub request_id: String,
    pub operation: CompatibilityOperation,
    pub budgets: WorkerBudgets,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CompatibilityOperation {
    Capabilities,
    Probe {
        source_path: String,
    },
    Transcode {
        source_path: String,
        output_root: String,
        output_path: String,
        profile: CompatibilityProfile,
    },
    ExtractFrames {
        source_path: String,
        output_root: String,
        width: u32,
        height: u32,
        frames: Vec<CompatibilityFrameRequest>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityFrameRequest {
    pub time_seconds: f64,
    pub output_path: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompatibilityProfile {
    ProRes422PcmMov,
    H264PcmMov,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerBudgets {
    pub timeout_millis: u64,
    pub max_output_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WorkerEvent {
    Progress {
        request_id: String,
        stage: String,
    },
    Completed {
        request_id: String,
        result: CompatibilityResult,
    },
    Failed {
        request_id: String,
        error: WorkerError,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(
    tag = "mode",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CompatibilityResult {
    Capabilities {
        protocol: String,
        schema_version: u32,
        factories: Vec<String>,
    },
    Probe {
        probe: MediaProbe,
    },
    Transcode {
        output_path: String,
        bytes: u64,
        probe: MediaProbe,
    },
    ExtractFrames {
        frames: Vec<CompatibilityFrameResult>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompatibilityFrameResult {
    pub time_seconds: f64,
    pub output_path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MediaProbe {
    pub duration_seconds: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<VideoProbe>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioProbe>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoProbe {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
    /// True when the stream is a single still image rather than timed video.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub still_image: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioProbe {
    pub sample_rate: u32,
    pub channels: u32,
    pub codec: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkerError {
    pub code: String,
    pub message: String,
    pub stage: String,
}

impl CompatibilityRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol != COMPATIBILITY_PROTOCOL_NAME
            || self.schema_version != COMPATIBILITY_PROTOCOL_VERSION
        {
            return Err("unsupported compatibility protocol".into());
        }
        if self.request_id.trim().is_empty()
            || self.budgets.timeout_millis == 0
            || self.budgets.max_output_bytes == 0
        {
            return Err("invalid compatibility request budgets or id".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_rejects_unknown_fields() {
        let value = serde_json::json!({"protocol":COMPATIBILITY_PROTOCOL_NAME,"schemaVersion":1,"requestId":"x","operation":{"mode":"capabilities"},"budgets":{"timeoutMillis":1,"maxOutputBytes":1},"secret":true});
        assert!(serde_json::from_value::<CompatibilityRequest>(value).is_err());
    }

    #[test]
    fn frame_extraction_contract_round_trips_ordered_bmp_outputs() {
        let request = CompatibilityRequest {
            protocol: COMPATIBILITY_PROTOCOL_NAME.into(),
            schema_version: COMPATIBILITY_PROTOCOL_VERSION,
            request_id: "filmstrip".into(),
            operation: CompatibilityOperation::ExtractFrames {
                source_path: "/project/media/input.mp4".into(),
                output_root: "/project/cache/filmstrip".into(),
                width: 120,
                height: 68,
                frames: vec![
                    CompatibilityFrameRequest {
                        time_seconds: 0.5,
                        output_path: "/project/cache/filmstrip/frame-000.bmp".into(),
                    },
                    CompatibilityFrameRequest {
                        time_seconds: 1.5,
                        output_path: "/project/cache/filmstrip/frame-001.bmp".into(),
                    },
                ],
            },
            budgets: WorkerBudgets {
                timeout_millis: 10_000,
                max_output_bytes: 1_000_000,
            },
        };
        let encoded = serde_json::to_value(&request).expect("serialize request");
        assert_eq!(encoded["operation"]["mode"], "extractFrames");
        assert_eq!(encoded["operation"]["frames"][1]["timeSeconds"], 1.5);
        assert_eq!(
            serde_json::from_value::<CompatibilityRequest>(encoded).expect("round trip"),
            request,
        );
    }
}

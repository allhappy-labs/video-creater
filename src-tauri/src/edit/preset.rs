use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EditPreset {
    TrailerCut,
    HighlightReel,
    StoryCut,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LanguageMode {
    Auto,
    #[serde(rename = "uk")]
    Ukrainian,
    #[serde(rename = "en")]
    English,
}

impl LanguageMode {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Ukrainian => "uk",
            Self::English => "en",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaptionStyle {
    Bold,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EditJobRequest {
    pub media_id: String,
    pub preset: EditPreset,
    pub prompt: String,
    pub target_duration_seconds: Option<f64>,
    pub language_mode: LanguageMode,
    pub caption_style: CaptionStyle,
    pub created_at: String,
}

#[derive(Debug, Error, PartialEq)]
pub enum EditRequestError {
    #[error("media id is required")]
    MediaIdRequired,
    #[error("prompt is required")]
    PromptRequired,
    #[error("target duration must be finite and greater than zero")]
    InvalidTargetDuration,
    #[error("target duration is outside preset range")]
    TargetDurationOutOfRange,
}

impl EditPreset {
    pub fn duration_range(self) -> (f64, f64) {
        match self {
            Self::TrailerCut => (30.0, 60.0),
            Self::HighlightReel => (45.0, 90.0),
            Self::StoryCut => (90.0, 180.0),
        }
    }

    pub fn default_prompt(self) -> &'static str {
        match self {
            Self::TrailerCut => {
                "Create a cinematic trailer cut with a strong hook, dramatic pacing, bold captions, and title-card moments."
            }
            Self::HighlightReel => {
                "Create a creator-native highlight reel that keeps the best spoken, visual, funny, or action moments."
            }
            Self::StoryCut => {
                "Create a longer story cut that preserves narrative context with readable captions and clear beat labels."
            }
        }
    }

    pub fn default_target_duration_seconds(self) -> f64 {
        let (min, max) = self.duration_range();
        (min + max) / 2.0
    }
}

impl EditJobRequest {
    pub fn validate(&self) -> Result<(), EditRequestError> {
        if self.media_id.trim().is_empty() {
            return Err(EditRequestError::MediaIdRequired);
        }

        if self.prompt.trim().is_empty() {
            return Err(EditRequestError::PromptRequired);
        }

        if let Some(duration) = self.target_duration_seconds {
            if !duration.is_finite() || duration <= 0.0 {
                return Err(EditRequestError::InvalidTargetDuration);
            }

            let (min, max) = self.preset.duration_range();
            if duration < min || duration > max {
                return Err(EditRequestError::TargetDurationOutOfRange);
            }
        }

        Ok(())
    }
}

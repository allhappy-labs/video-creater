//! Model file layout expected by the helper. The app downloads and verifies these files
//! (`src-tauri/src/transcription/model.rs` and `src-tauri/src/speech_models.rs`); the helper only
//! locates them.

use std::path::{Path, PathBuf};

use crate::protocol::HelperError;

pub const PARAKEET_ENCODER: &str = "encoder.int8.onnx";
pub const PARAKEET_DECODER: &str = "decoder.int8.onnx";
pub const PARAKEET_JOINER: &str = "joiner.int8.onnx";
pub const PARAKEET_TOKENS: &str = "tokens.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpeechModelFile {
    pub repo_dir: &'static str,
    pub file_name: &'static str,
}

pub const SILERO_VAD: SpeechModelFile = SpeechModelFile {
    repo_dir: "silero-vad",
    file_name: "silero_vad.onnx",
};
pub const PYANNOTE_SEGMENTATION: SpeechModelFile = SpeechModelFile {
    repo_dir: "pyannote-segmentation-3-0",
    file_name: "model.onnx",
};
pub const WESPEAKER_EMBEDDING: SpeechModelFile = SpeechModelFile {
    repo_dir: "speaker-embedding-models",
    file_name: "wespeaker_en_voxceleb_resnet34_LM.onnx",
};

pub struct ParakeetPaths {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
}

pub fn parakeet_paths(model_dir: &Path) -> Result<ParakeetPaths, HelperError> {
    if !model_dir.is_dir() {
        return Err(HelperError::MissingModelDirectory(
            model_dir.display().to_string(),
        ));
    }
    let require = |name: &str| {
        let path = model_dir.join(name);
        if path.is_file() {
            Ok(path)
        } else {
            Err(HelperError::MissingModelFile(
                name.to_string(),
                model_dir.display().to_string(),
            ))
        }
    };
    Ok(ParakeetPaths {
        encoder: require(PARAKEET_ENCODER)?,
        decoder: require(PARAKEET_DECODER)?,
        joiner: require(PARAKEET_JOINER)?,
        tokens: require(PARAKEET_TOKENS)?,
    })
}

/// Mirrors the Swift `locateCompiledModel`/`locateFile` search: the file directly under the root
/// or under the repository folder inside the root.
pub fn locate_speech_model(root: &str, model: SpeechModelFile) -> Result<PathBuf, HelperError> {
    let root_path = Path::new(root);
    if !root_path.is_dir() {
        return Err(HelperError::MissingModelRoot(root.to_string()));
    }
    [
        root_path.join(model.file_name),
        root_path.join(model.repo_dir).join(model.file_name),
    ]
    .into_iter()
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| {
        HelperError::MissingModelFile(model.file_name.to_string(), root_path.display().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parakeet_paths_require_every_onnx_file() {
        let directory = tempfile::tempdir().expect("model dir");
        for name in [PARAKEET_ENCODER, PARAKEET_DECODER, PARAKEET_JOINER] {
            std::fs::write(directory.path().join(name), b"onnx").expect("write model file");
        }
        assert_eq!(
            parakeet_paths(directory.path()).err(),
            Some(HelperError::MissingModelFile(
                PARAKEET_TOKENS.to_string(),
                directory.path().display().to_string()
            ))
        );
        std::fs::write(directory.path().join(PARAKEET_TOKENS), b"<blk> 0").expect("tokens");
        assert!(parakeet_paths(directory.path()).is_ok());
        assert_eq!(
            parakeet_paths(&directory.path().join("missing")).err(),
            Some(HelperError::MissingModelDirectory(
                directory.path().join("missing").display().to_string()
            ))
        );
    }

    #[test]
    fn speech_models_are_found_in_the_root_or_repository_folder() {
        let directory = tempfile::tempdir().expect("speech root");
        let root = directory.path().to_string_lossy().to_string();
        assert!(matches!(
            locate_speech_model(&root, SILERO_VAD),
            Err(HelperError::MissingModelFile(..))
        ));
        let nested = directory.path().join(SILERO_VAD.repo_dir);
        std::fs::create_dir_all(&nested).expect("repo dir");
        std::fs::write(nested.join(SILERO_VAD.file_name), b"onnx").expect("vad");
        assert_eq!(
            locate_speech_model(&root, SILERO_VAD).expect("nested vad"),
            nested.join(SILERO_VAD.file_name)
        );
        assert_eq!(
            locate_speech_model("/definitely/missing/root", SILERO_VAD).err(),
            Some(HelperError::MissingModelRoot(
                "/definitely/missing/root".to_string()
            ))
        );
    }
}

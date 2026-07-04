use super::audio_id::AudioId;
use super::audio_kind::AudioKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioReference {
    pub audio_id:   AudioId,
    pub audio_kind: AudioKind,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("message deserialization failed: {0}")]
    MsgDeserFailed(String),
    #[error("message serialization {0}")]
    MsgSerFailed(String),
}

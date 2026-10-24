//! Admission control and conservative size checks around the shared session parsers.
use crate::session_manager::{self, content::SourceLocation, LoadedTranscript};
use std::{
    io::Read,
    sync::{Arc, LazyLock},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PAYLOAD_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_MESSAGE_BYTES: usize = 2 * 1024 * 1024;
static READS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(2)));

pub fn acquire() -> Result<Arc<OwnedSemaphorePermit>, String> {
    READS
        .clone()
        .try_acquire_owned()
        .map(Arc::new)
        .map_err(|_| "E_BUSY: 会话读取繁忙，请稍后重试".into())
}

/// The worker retains its permit even if the HTTP future is cancelled.
pub async fn run<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    let permit = acquire()?;
    super::routes::blocking(move || {
        let _permit = permit;
        work()
    })
    .await
}

pub fn load(provider: &str, path: &str) -> Result<LoadedTranscript, String> {
    let started = std::time::Instant::now();
    let source = session_manager::content::validate_source(provider, path)?;
    let fingerprint = session_manager::cache::source_fingerprint(&source)
        .map_err(|_| "E_SESSION_READ: 无法读取会话元数据".to_string())?;
    let cached = session_manager::cache::global().get(
        &(source.provider_id.clone(), source.raw.clone()),
        fingerprint,
    );
    if let SourceLocation::Path { path, .. } = &source.location {
        if fingerprint.len > MAX_SOURCE_BYTES {
            return Err("E_SESSION_LIMIT: 会话源超过 64 MiB，请在 CLI 中查看".into());
        }
        if cached.is_none() && crate::codex_rollout_file::is_compressed(path) {
            // Bound decompressed input too; do not allocate a decompressed String here.
            let mut reader = crate::codex_rollout_file::open_reader(path)
                .map_err(|_| "E_SESSION_READ: 无法读取压缩会话".to_string())?
                .take(MAX_SOURCE_BYTES + 1);
            let bytes = std::io::copy(&mut reader, &mut std::io::sink())
                .map_err(|_| "E_SESSION_READ: 压缩会话读取失败".to_string())?;
            if bytes > MAX_SOURCE_BYTES {
                return Err("E_SESSION_LIMIT: 会话解压后超过 64 MiB，请在 CLI 中查看".into());
            }
        }
    }
    // SQLite sources are bounded by concurrency and output, not the whole DB size.
    // Parsers still load the complete transcript: these are not hard memory limits.
    let loaded = match cached {
        Some(transcript) => LoadedTranscript {
            transcript,
            cached: true,
            parse_ms: started.elapsed().as_millis() as u64,
        },
        None => session_manager::load_transcript(provider, path)?,
    };
    if loaded.transcript.approx_bytes > MAX_PAYLOAD_BYTES
        || loaded
            .transcript
            .message_bytes
            .iter()
            .any(|bytes| *bytes > MAX_MESSAGE_BYTES)
    {
        session_manager::cache::global().remove(provider, path);
        return Err(
            "E_SESSION_LIMIT: 会话结果超过 32 MiB 或单条消息超过 2 MiB，请在 CLI 中查看".into(),
        );
    }
    Ok(loaded)
}

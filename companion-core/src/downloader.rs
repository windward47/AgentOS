//! Model downloader — fetches Live2D model zip from a URL and extracts to
//! `~/.companion/models/{model_name}/`.

use std::path::PathBuf;
use tokio::sync::mpsc;

/// Progress event emitted during download + extraction.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "phase")]
pub enum DownloadProgress {
    /// Download has started. `.total` is the content-length in bytes (0 if unknown).
    #[serde(rename = "downloading")]
    Downloading { downloaded: u64, total: u64 },
    /// Download complete, extracting zip.
    #[serde(rename = "extracting")]
    Extracting,
    /// All done — model is ready.
    #[serde(rename = "done")]
    Done { model_id: String },
    /// Something went wrong.
    #[serde(rename = "error")]
    Error { message: String },
}

/// Download a model zip from `url`, extract it into `dest_dir/{model_id}/`,
/// and emit progress via the given `tx` channel.
pub async fn download_model(
    url: &str,
    model_id: &str,
    dest_root: &PathBuf,
    tx: mpsc::Sender<DownloadProgress>,
) -> Result<(), String> {
    let dest_dir = dest_root.join(model_id);
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("mkdir: {e}"))?;

    // ── Download ──────────────────────────────────────────────────────
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // 10 minutes for large models
        .build()
        .map_err(|e| format!("build client: {e}"))?;

    let resp = client.get(url).send().await.map_err(|e| format!("GET {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);

    let mut downloaded: u64 = 0;
    let mut buf = Vec::with_capacity(total as usize);
    let mut stream = resp.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("download chunk: {e}"))?;
        downloaded += chunk.len() as u64;
        buf.extend_from_slice(&chunk);
        let _ = tx.send(DownloadProgress::Downloading { downloaded, total }).await;
    }

    // ── Extract ───────────────────────────────────────────────────────
    let _ = tx.send(DownloadProgress::Extracting).await;

    let cursor = std::io::Cursor::new(&buf);
    let mut archive = zip::ZipArchive::new(cursor).map_err(|e| format!("open zip: {e}"))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("read zip entry {i}: {e}"))?;
        let name = file.mangled_name();

        // Skip macOS resource forks and __MACOSX
        let name_str = name.to_string_lossy();
        if name_str.contains("__MACOSX") || name_str.starts_with('.') {
            continue;
        }

        let out_path = if let Some(rest) = name_str.strip_prefix(model_id) {
            // If the zip already contains a top-level model_id directory, strip it
            let stripped = rest.trim_start_matches('/').trim_start_matches('\\');
            if stripped.is_empty() { continue; }
            dest_dir.join(stripped)
        } else if name_str.contains('/') {
            // Relative path inside zip — extract as-is under dest_dir
            dest_dir.join(&*name_str)
        } else {
            dest_dir.join(&*name_str)
        };

        if file.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| format!("mkdir {out_path:?}: {e}"))?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("mkdir {parent:?}: {e}"))?;
            }
            let mut f = std::fs::File::create(&out_path).map_err(|e| format!("create {out_path:?}: {e}"))?;
            std::io::copy(&mut file, &mut f).map_err(|e| format!("write {out_path:?}: {e}"))?;
        }
    }

    let _ = tx.send(DownloadProgress::Done { model_id: model_id.to_string() }).await;
    Ok(())
}

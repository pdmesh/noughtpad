//! Resumable, cancellable, checksum-verified model downloads.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use crate::catalog::LocalModel;
use crate::{Error, Result};

/// Where a model's file lives: `<models>/<id>/<file>`.
pub fn model_path(models: &Path, m: &LocalModel) -> PathBuf {
    models.join(m.id).join(m.file)
}

pub fn is_downloaded(models: &Path, m: &LocalModel) -> bool {
    model_path(models, m).metadata().is_ok_and(|md| md.len() == m.bytes)
}

/// Bytes of a partial download already on disk (to resume from).
pub fn partial_bytes(models: &Path, m: &LocalModel) -> u64 {
    part_path(models, m).metadata().map_or(0, |md| md.len())
}

fn part_path(models: &Path, m: &LocalModel) -> PathBuf {
    models.join(m.id).join(format!("{}.part", m.file))
}

/// Delete a model (and any partial download).
pub fn remove(models: &Path, m: &LocalModel) -> Result<()> {
    let dir = models.join(m.id);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
    }
    Ok(())
}

/// Download `m` into `models`, resuming a partial file. `progress(received, total)`
/// is called every ~1 MB. Returns `Error::Cancelled` if `cancel` is set; the
/// partial file is kept so the next call resumes.
pub fn download(models: &Path, m: &LocalModel, cancel: &AtomicBool, progress: impl FnMut(u64, u64)) -> Result<PathBuf> {
    download_from(&m.url(), models, m, cancel, progress)
}

pub(crate) fn download_from(
    url: &str,
    models: &Path,
    m: &LocalModel,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<PathBuf> {
    let dest = model_path(models, m);
    if is_downloaded(models, m) {
        return Ok(dest);
    }
    let dir = models.join(m.id);
    fs::create_dir_all(&dir).map_err(|e| Error::Io(format!("{}: {e}", dir.display())))?;
    let part = part_path(models, m);
    let mut have = partial_bytes(models, m);
    if have > m.bytes {
        fs::remove_file(&part).ok();
        have = 0;
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(ureq::Proxy::try_from_env())
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = agent.get(url);
    if have > 0 {
        req = req.header("Range", &format!("bytes={have}-"));
    }
    let resp = req.call().map_err(|e| Error::Download(e.to_string()))?;
    let status = resp.status().as_u16();
    let resumed = match status {
        206 => true,
        200 => false,
        416 if have == m.bytes => true,
        _ => return Err(Error::Download(format!("{} answered HTTP {status}", host(url)))),
    };
    if !resumed {
        have = 0;
    }
    let mut out = OpenOptions::new()
        .create(true)
        .write(true)
        .append(resumed)
        .truncate(!resumed)
        .open(&part)
        .map_err(|e| Error::Io(format!("{}: {e}", part.display())))?;

    let mut body = resp.into_body().into_reader();
    let mut buf = vec![0u8; 256 * 1024];
    let mut last = have;
    progress(have, m.bytes);
    while have < m.bytes {
        if cancel.load(Ordering::Relaxed) {
            out.sync_all().ok();
            return Err(Error::Cancelled);
        }
        let n = body.read(&mut buf).map_err(|e| Error::Download(e.to_string()))?;
        if n == 0 {
            break;
        }
        out.write_all(&buf[..n]).map_err(|e| Error::Io(e.to_string()))?;
        have += n as u64;
        if have - last >= 1 << 20 || have == m.bytes {
            last = have;
            progress(have, m.bytes);
        }
    }
    out.sync_all().map_err(|e| Error::Io(e.to_string()))?;
    drop(out);
    if have != m.bytes {
        return Err(Error::Download(format!(
            "connection closed after {have} of {} bytes; try again to resume",
            m.bytes
        )));
    }

    // Verify before the file can be used.
    let digest = sha256_file(&part, cancel)?;
    if digest != m.sha256 {
        fs::remove_file(&part).ok();
        return Err(Error::Download(
            "the downloaded file didn't match its checksum and was deleted; try again".into(),
        ));
    }
    fs::rename(&part, &dest).map_err(|e| Error::Io(e.to_string()))?;
    Ok(dest)
}

fn sha256_file(path: &Path, cancel: &AtomicBool) -> Result<String> {
    let mut f = File::open(path).map_err(|e| Error::Io(e.to_string()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let n = f.read(&mut buf).map_err(|e| Error::Io(e.to_string()))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn host(url: &str) -> &str {
    url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::net::TcpListener;

    /// A tiny HTTP server that serves `data`, honouring `Range`, and stops
    /// after `cut` bytes on the first request (to test resuming).
    fn serve(data: Vec<u8>, cut: Option<usize>) -> String {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut first = true;
            for stream in l.incoming() {
                let mut s = stream.unwrap();
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut from = 0usize;
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if line == "\r\n" || line.is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_ascii_lowercase().strip_prefix("range: bytes=") {
                        from = v.trim().trim_end_matches('-').parse().unwrap();
                    }
                }
                let body = &data[from..];
                let (status, body) = match (first, cut) {
                    (true, Some(c)) => ("200 OK", &body[..c]),
                    _ if from > 0 => ("206 Partial Content", body),
                    _ => ("200 OK", body),
                };
                // Advertise the full length so a cut looks like a dropped connection.
                let len = if first && cut.is_some() { data.len() } else { body.len() };
                let _ = write!(
                    s,
                    "HTTP/1.1 {status}\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
                );
                let _ = s.write_all(body);
                first = false;
            }
        });
        format!("http://{addr}/model.gguf")
    }

    fn model(data: &[u8]) -> LocalModel {
        let digest: String = Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect();
        LocalModel {
            id: "test",
            name: "Test",
            maker: "",
            licence: "",
            repo: "",
            revision: "",
            file: "model.gguf",
            sha256: Box::leak(digest.into_boxed_str()),
            bytes: data.len() as u64,
            ram_gb: 0.0,
            blurb: "",
            recommended: false,
        }
    }

    #[test]
    fn downloads_resumes_and_verifies() {
        let data: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        let m = model(&data);
        let dir = tempfile::tempdir().unwrap();
        let url = serve(data.clone(), Some(1_200_000));
        let cancel = AtomicBool::new(false);

        // First attempt drops mid-way and keeps the partial file.
        let err = download_from(&url, dir.path(), &m, &cancel, |_, _| {}).unwrap_err();
        assert!(matches!(err, Error::Download(_)), "{err:?}");
        assert_eq!(partial_bytes(dir.path(), &m), 1_200_000);

        // Second attempt resumes from there.
        let mut seen = vec![];
        let path = download_from(&url, dir.path(), &m, &cancel, |r, _| seen.push(r)).unwrap();
        assert_eq!(seen[0], 1_200_000);
        assert_eq!(fs::read(&path).unwrap(), data);
        assert!(is_downloaded(dir.path(), &m));
        remove(dir.path(), &m).unwrap();
        assert!(!is_downloaded(dir.path(), &m));
    }

    #[test]
    fn rejects_a_bad_checksum() {
        let data = vec![7u8; 10_000];
        let mut m = model(&data);
        m.sha256 = "0000000000000000000000000000000000000000000000000000000000000000";
        let dir = tempfile::tempdir().unwrap();
        let url = serve(data, None);
        let err = download_from(&url, dir.path(), &m, &AtomicBool::new(false), |_, _| {}).unwrap_err();
        assert!(err.to_string().contains("checksum"));
        assert_eq!(partial_bytes(dir.path(), &m), 0);
    }

    #[test]
    fn cancels() {
        let data = vec![1u8; 5_000_000];
        let m = model(&data);
        let dir = tempfile::tempdir().unwrap();
        let url = serve(data, None);
        let err = download_from(&url, dir.path(), &m, &AtomicBool::new(true), |_, _| {}).unwrap_err();
        assert!(matches!(err, Error::Cancelled));
    }
}

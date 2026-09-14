use super::*;

fn fixture() -> (tempfile::TempDir, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Workspace::new(dir.path().to_path_buf()).unwrap();
    (dir, workspace)
}

// A complete synthetic one-page PDF with correct xref offsets.
fn sample_pdf() -> Vec<u8> {
    let stream = "BT /F1 12 Tf 72 720 Td (Synthetic PDF transfer test) Tj ET";
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".to_string(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
    ];
    let mut pdf = "%PDF-1.4\n".to_string();
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{obj}\nendobj\n", i + 1));
    }
    let xref = pdf.len();
    pdf.push_str("xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets { pdf.push_str(&format!("{offset:010} 00000 n \n")); }
    pdf.push_str(&format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n"));
    pdf.into_bytes()
}

#[test]
fn rejects_invalid_files_and_accepts_size_boundary() {
    let (dir, ws) = fixture();
    assert!(read_pdf(&ws, "").is_err());
    assert!(read_pdf(&ws, "missing.pdf").is_err());
    std::fs::create_dir(dir.path().join("dir.pdf")).unwrap();
    assert!(read_pdf(&ws, "dir.pdf").is_err());
    std::fs::write(dir.path().join("fake.pdf"), "hello").unwrap();
    assert_eq!(read_pdf(&ws, "fake.pdf").err().unwrap().0, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    std::fs::write(dir.path().join("text.txt"), sample_pdf()).unwrap();
    assert!(read_pdf(&ws, "text.txt").is_err());
    let mut bytes = vec![b' '; MAX_PDF_BYTES];
    bytes[..5].copy_from_slice(b"%PDF-");
    std::fs::write(dir.path().join("limit.pdf"), &bytes).unwrap();
    assert_eq!(read_pdf(&ws, "limit.pdf").unwrap().bytes.len(), MAX_PDF_BYTES);
    bytes.push(b' ');
    std::fs::write(dir.path().join("limit.pdf"), &bytes).unwrap();
    assert_eq!(read_pdf(&ws, "limit.pdf").err().unwrap().0, StatusCode::PAYLOAD_TOO_LARGE);
}

#[test]
fn cache_expires_and_enforces_capacity_without_evicting_active_links() {
    let mut cache = PdfCache::default();
    let file = || PdfFile {bytes: Bytes::from(vec![0; MAX_PDF_BYTES]), disposition: "attachment".into(), expires: Instant::now() + TTL};
    let first = cache.insert(file()).unwrap();
    cache.insert(file()).unwrap();
    cache.insert(file()).unwrap();
    assert_eq!(cache.insert(file()).err().unwrap().0, StatusCode::TOO_MANY_REQUESTS);
    assert!(cache.entries.contains_key(&first));
    cache.entries.get_mut(&first).unwrap().expires = Instant::now();
    cache.prune();
    assert!(!cache.entries.contains_key(&first));
    cache.insert(file()).unwrap();
}

#[test]
fn validates_configured_urls() {
    for url in ["https://files.example.com", "https://files.example.com/prefix", "http://127.0.0.1:4567", "http://[::1]:4567"] {
        assert!(validate_base_url(url).is_ok(), "{url}");
    }
    for url in ["http://files.example.com", "file:///tmp", "https://user:pass@example.com", "https://example.com/?x=1", "https://example.com/#foo"] {
        assert!(validate_base_url(url).is_err(), "{url}");
    }
}

#[cfg(unix)]
#[test]
fn preserves_read_path_boundary() {
    let (dir, ws) = fixture();
    let outside = tempfile::tempdir().unwrap();
    let path = outside.path().join("outside.pdf");
    std::fs::write(&path, sample_pdf()).unwrap();
    std::os::unix::fs::symlink(&path, dir.path().join("escape.pdf")).unwrap();
    assert!(read_pdf(&ws, "escape.pdf").is_err());
    assert!(read_pdf(&ws, path.to_str().unwrap()).is_ok());
}

#[tokio::test]
async fn expired_and_unknown_downloads_are_not_found() {
    let (_dir, workspace) = fixture();
    let state = PdfState {workspace, base_url: "http://127.0.0.1".into(), cache: Arc::new(Mutex::new(PdfCache::default())), read_gate: Arc::new(Semaphore::new(1))};
    state.cache.lock().await.entries.insert("expired".into(), PdfFile {
        bytes: Bytes::from_static(b"%PDF-1.4"), disposition: "attachment".into(), expires: Instant::now(),
    });
    for token in ["expired", "unknown"] {
        assert_eq!(download_pdf(State(state.clone()), Path(token.to_string())).await.err().unwrap().0, StatusCode::NOT_FOUND);
    }
}

#[test]
fn openapi_uses_top_level_file_contract_and_retains_tool_contracts() {
    let schema = super::super::openapi::build_openapi(&crate::tools::list_tools(), "https://example.com", "api_key");
    let post = &schema["paths"]["/actions/return_pdf"]["post"];
    assert_eq!(post["security"], json!([{"bearerAuth": []}]));
    assert_eq!(post["responses"]["200"]["content"]["application/json"]["schema"]["required"], json!(["openaiFileResponse"]));
    assert_eq!(schema["paths"]["/actions/read_file"]["post"]["responses"]["200"]["content"]["application/json"]["schema"]["$ref"], "#/components/schemas/ToolExecutionResponse");
    assert!(schema["paths"]["/action-files/{token}"].is_null());
}

#[tokio::test]
async fn http_auth_file_return_headers_snapshot_and_retry() {
    let (dir, ws) = fixture();
    let original = sample_pdf();
    let filename = "测试 report.pdf";
    std::fs::write(dir.path().join(filename), &original).unwrap();
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = socket.local_addr().unwrap().port();
    let auth = Arc::new(AuthConfig::new("api_key".into(), Some("test-secret".into()), None, port, String::new()));
    // Exercise the same static + generic route merge used by the full listener.
    let generic = Router::new().route("/actions/{tool_name}", post(|| async {"generic"}));
    let app = generic.merge(router(ws, auth));
    let task = tokio::spawn(async move { axum::serve(socket, app).await.unwrap(); });
    let client = reqwest::Client::new();
    let endpoint = format!("http://127.0.0.1:{port}/actions/return_pdf");
    let no_auth = client.post(&endpoint).json(&json!({"path":filename})).send().await.unwrap();
    assert_eq!(no_auth.status(), StatusCode::UNAUTHORIZED);
    let wrong = client.post(&endpoint).bearer_auth("wrong").json(&json!({"path":filename})).send().await.unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let response = client.post(&endpoint).bearer_auth("test-secret").header("Host", "attacker.example").json(&json!({"path":filename})).send().await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let result: Value = response.json().await.unwrap();
    assert_eq!(result.as_object().unwrap().len(), 1);
    let url = result["openaiFileResponse"][0].as_str().unwrap();
    assert!(url.starts_with(&format!("http://127.0.0.1:{port}/action-files/")));
    std::fs::write(dir.path().join(filename), b"changed").unwrap();
    for _ in 0..2 {
        let file = client.get(url).send().await.unwrap();
        assert_eq!(file.status(), StatusCode::OK);
        assert_eq!(file.headers()["content-type"], "application/pdf");
        assert_eq!(file.headers()["cache-control"], "no-store");
        assert!(file.headers()["content-disposition"].to_str().unwrap().contains("filename*=UTF-8''%E6%B5%8B%E8%AF%95%20report.pdf"));
        assert_eq!(file.bytes().await.unwrap().as_ref(), original.as_slice());
    }
    let missing = client.get(format!("http://127.0.0.1:{port}/action-files/invalid")).send().await.unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    task.abort();
}


#[tokio::test]
async fn timed_out_pdf_worker_holds_its_permit_until_it_really_finishes() {
    use std::sync::atomic::{AtomicBool, Ordering};
    let gate = Arc::new(Semaphore::new(1));
    let started = Arc::new(AtomicBool::new(false));
    let signal = started.clone();
    let (release, blocked) = std::sync::mpsc::channel();
    let task = tokio::spawn(prepare_bounded(gate.clone(), Duration::from_millis(100), move || {
        signal.store(true, Ordering::SeqCst);
        let _ = blocked.recv_timeout(Duration::from_secs(2));
        Ok(PdfFile { bytes: Bytes::from_static(b"%PDF-"), disposition: "attachment".into(), expires: Instant::now() + TTL })
    }));
    let result = task.await.unwrap();
    assert!(started.load(Ordering::SeqCst));
    assert_eq!(result.err().unwrap().0, StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(gate.available_permits(), 0);
    let second = Arc::new(AtomicBool::new(false));
    let check = second.clone();
    let result = prepare_bounded(gate.clone(), Duration::from_millis(20), move || {
        check.store(true, Ordering::SeqCst);
        Err(error(StatusCode::BAD_REQUEST, "Should not run"))
    }).await;
    assert_eq!(result.err().unwrap().0, StatusCode::GATEWAY_TIMEOUT);
    assert!(!second.load(Ordering::SeqCst));
    release.send(()).unwrap();
    let permit = tokio::time::timeout(Duration::from_secs(1), gate.acquire_owned()).await.unwrap().unwrap();
    drop(permit);
}

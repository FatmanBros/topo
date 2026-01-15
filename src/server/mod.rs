//! Server module - HTTP server and dev server with hot reload

use anyhow::Result;
use notify::{Watcher, RecursiveMode};
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tiny_http::{Header, Response, Server};
use tungstenite::{accept, Message};

/// Create Content-Type header safely (infallible for valid ASCII)
fn content_type_header(content_type: &[u8]) -> Header {
    Header::from_bytes(&b"Content-Type"[..], content_type)
        .expect("Content-Type header with valid ASCII should never fail")
}

/// Create CORS header safely
fn cors_header() -> Header {
    Header::from_bytes(&b"Access-Control-Allow-Origin"[..], b"*")
        .expect("CORS header with valid ASCII should never fail")
}

use topo::config::{Config, BuildMode};
use crate::build;

pub fn start_server(port: u16, output_dir: &PathBuf, open_browser: bool, base_path: &str) -> Result<()> {
    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("Address already in use") || err_str.contains("os error 98") {
            anyhow::anyhow!(
                "Port {} is already in use.\n\n\
                 Try one of the following:\n\
                 • Stop the other process using port {}\n\
                 • Use a different port: topo start --port {}\n\
                 • Kill the process: lsof -ti:{} | xargs kill -9",
                port, port, port + 1, port
            )
        } else {
            anyhow::anyhow!("Failed to start server: {}", e)
        }
    })?;

    println!();
    println!("  Server running at:");
    println!("  Local:   http://localhost:{}", port);
    println!();
    println!("  Press Ctrl+C to stop");
    println!();

    if open_browser {
        let url = format!("http://localhost:{}", port);
        if let Err(e) = open_in_browser(&url) {
            eprintln!("  Warning: Could not open browser: {}", e);
        }
    }

    for request in server.incoming_requests() {
        let raw_url_path = request.url().trim_start_matches('/');

        let url_path = if !base_path.is_empty() {
            let bp = base_path.trim_start_matches('/');
            if raw_url_path.starts_with(bp) {
                raw_url_path.strip_prefix(bp)
                    .unwrap_or(raw_url_path)
                    .trim_start_matches('/')
            } else {
                raw_url_path
            }
        } else {
            raw_url_path
        };

        let file_path = if url_path.is_empty() || url_path == "/" {
            Some(output_dir.join("index.html"))
        } else {
            safe_resolve_path(output_dir, url_path)
        };

        let response = match file_path {
            Some(path) if path.exists() && path.is_file() => {
                match fs::read(&path) {
                    Ok(content) => {
                        let content_type = get_content_type(&path);
                        Response::from_data(content)
                            .with_header(content_type_header(content_type.as_bytes()))
                    }
                    Err(_) => Response::from_string("500 Internal Server Error")
                        .with_status_code(500),
                }
            }
            _ => {
                match fs::read(output_dir.join("index.html")) {
                    Ok(content) => Response::from_data(content)
                        .with_header(content_type_header(b"text/html")),
                    Err(_) => Response::from_string("404 Not Found").with_status_code(404),
                }
            }
        };

        let _ = request.respond(response);
    }

    Ok(())
}

fn safe_resolve_path(base: &PathBuf, url_path: &str) -> Option<PathBuf> {
    let base_canonical = match base.canonicalize() {
        Ok(p) => p,
        Err(_) => return None,
    };

    if url_path.contains("..") || url_path.contains('\0') {
        return None;
    }

    let file_path = base.join(url_path);

    if !file_path.exists() {
        let normalized = file_path.components()
            .fold(PathBuf::new(), |mut path, comp| {
                match comp {
                    std::path::Component::ParentDir => { path.pop(); }
                    std::path::Component::Normal(s) => { path.push(s); }
                    std::path::Component::RootDir => { path.push("/"); }
                    _ => {}
                }
                path
            });
        if !normalized.starts_with(&base_canonical) && !base.join(&normalized).starts_with(base) {
            return None;
        }
        return Some(file_path);
    }

    match file_path.canonicalize() {
        Ok(resolved) if resolved.starts_with(&base_canonical) => Some(resolved),
        _ => None,
    }
}

fn get_content_type(path: &Path) -> String {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn open_in_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()?;
    }
    Ok(())
}

pub fn start_dev_server(port: u16, config: &Config) -> Result<()> {
    let build_config = config.build_config();
    let paths_config = config.paths_config();
    let dev_config = config.dev_config();

    let input = PathBuf::from(&paths_config.pages);
    let output = PathBuf::from(&build_config.output);
    let mode = match build_config.mode {
        BuildMode::Spa => "spa".to_string(),
        BuildMode::Ssg => "ssg".to_string(),
        BuildMode::Ssr => "ssr".to_string(),
    };

    build::build_project_dev(&input, &output, &mode, port, config)?;

    let ws_clients: Arc<Mutex<Vec<std::net::TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
    let ws_clients_clone = Arc::clone(&ws_clients);

    let ws_port = port + 1;
    std::thread::spawn(move || {
        let listener = match TcpListener::bind(format!("0.0.0.0:{}", ws_port)) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("  Warning: Could not start WebSocket server: {}", e);
                return;
            }
        };

        for stream in listener.incoming().flatten() {
            let ws_clients = Arc::clone(&ws_clients_clone);
            std::thread::spawn(move || {
                let stream_clone = match stream.try_clone() {
                    Ok(s) => s,
                    Err(_) => return, // Skip if we can't clone the stream
                };
                if let Ok(mut websocket) = accept(stream_clone) {
                    if let Ok(mut clients) = ws_clients.lock() {
                        clients.push(stream);
                    }
                    loop {
                        match websocket.read() {
                            Ok(Message::Close(_)) | Err(_) => break,
                            Ok(Message::Ping(data)) => {
                                let _ = websocket.send(Message::Pong(data));
                            }
                            _ => {}
                        }
                    }
                }
            });
        }
    });

    let ws_clients_for_watcher = Arc::clone(&ws_clients);
    let input_clone = input.clone();
    let output_clone = output.clone();
    let mode_clone = mode.clone();
    let config_clone = config.clone();

    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
        if let Ok(event) = res {
            if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                let _ = tx.send(());
            }
        }
    })?;

    watcher.watch(&input, RecursiveMode::Recursive)?;

    let components_dir = PathBuf::from(&paths_config.components);
    if components_dir.exists() {
        watcher.watch(&components_dir, RecursiveMode::Recursive)?;
    }

    std::thread::spawn(move || {
        let mut last_rebuild = std::time::Instant::now();
        loop {
            if rx.recv().is_ok() {
                std::thread::sleep(Duration::from_millis(100));
                while rx.try_recv().is_ok() {}

                if last_rebuild.elapsed() < Duration::from_millis(200) {
                    continue;
                }

                println!("\n  File changed, rebuilding...");

                match build::build_project_dev(&input_clone, &output_clone, &mode_clone, port, &config_clone) {
                    Ok(_) => {
                        println!("  ✓ Rebuild complete");

                        if let Ok(mut clients) = ws_clients_for_watcher.lock() {
                            clients.retain(|client| {
                                let stream_clone = match client.try_clone() {
                                    Ok(s) => s,
                                    Err(_) => return false, // Remove client if we can't clone
                                };
                                if let Ok(mut ws) = accept(stream_clone) {
                                    ws.send(Message::Text("reload".into())).is_ok()
                                } else {
                                    false
                                }
                            });
                        }
                    }
                    Err(e) => {
                        eprintln!("  ✗ Build error: {}", e);
                    }
                }
                last_rebuild = std::time::Instant::now();
            }
        }
    });

    let addr = format!("0.0.0.0:{}", port);
    let server = Server::http(&addr).map_err(|e| {
        let err_str = e.to_string();
        if err_str.contains("Address already in use") || err_str.contains("os error 98") {
            anyhow::anyhow!(
                "Port {} is already in use.\n\n\
                 Try: topo dev --port {}",
                port, port + 10
            )
        } else {
            anyhow::anyhow!("Failed to start server: {}", e)
        }
    })?;

    println!();
    println!("  Dev server running at:");
    println!("  Local:     http://localhost:{}", port);
    println!("  WebSocket: ws://localhost:{}", ws_port);
    println!();
    println!("  Watching for file changes...");
    println!("  Press Ctrl+C to stop");
    println!();

    if dev_config.open {
        let url = format!("http://localhost:{}", port);
        if let Err(e) = open_in_browser(&url) {
            eprintln!("  Warning: Could not open browser: {}", e);
        }
    }

    let mocks_dir = input.parent().unwrap_or(&input).join("mocks");

    for request in server.incoming_requests() {
        let url_path = request.url().trim_start_matches('/');

        let response = if url_path.starts_with("api/") {
            serve_mock_api(url_path, &mocks_dir)
        } else {
            let file_path = if url_path.is_empty() || url_path == "/" {
                Some(output.join("index.html"))
            } else {
                safe_resolve_path(&output, url_path)
            };

            match file_path {
                Some(path) if path.exists() && path.is_file() => {
                    match fs::read(&path) {
                        Ok(content) => {
                            let content_type = get_content_type(&path);
                            Response::from_data(content)
                                .with_header(content_type_header(content_type.as_bytes()))
                        }
                        Err(_) => Response::from_string("500 Internal Server Error")
                            .with_status_code(500),
                    }
                }
                _ => {
                    match fs::read(output.join("index.html")) {
                        Ok(content) => Response::from_data(content)
                            .with_header(content_type_header(b"text/html")),
                        Err(_) => Response::from_string("404 Not Found").with_status_code(404),
                    }
                }
            }
        };

        let _ = request.respond(response);
    }

    Ok(())
}

fn serve_mock_api(url_path: &str, mocks_dir: &PathBuf) -> Response<std::io::Cursor<Vec<u8>>> {
    let api_path = url_path.strip_prefix("api/").unwrap_or(url_path);

    if api_path.contains("..") || api_path.contains('\0') {
        return Response::from_string(r#"{"error": "Invalid path"}"#)
            .with_status_code(400)
            .with_header(content_type_header(b"application/json"));
    }

    let mock_file = match safe_resolve_path(mocks_dir, &format!("{}.json", api_path)) {
        Some(path) => path,
        None => {
            return Response::from_string(r#"{"error": "Invalid path"}"#)
                .with_status_code(400)
                .with_header(content_type_header(b"application/json"));
        }
    };

    if mock_file.exists() {
        match fs::read(&mock_file) {
            Ok(content) => {
                Response::from_data(content)
                    .with_header(content_type_header(b"application/json"))
                    .with_header(cors_header())
            }
            Err(_) => Response::from_string(r#"{"error": "Failed to read mock file"}"#)
                .with_status_code(500)
                .with_header(content_type_header(b"application/json")),
        }
    } else {
        Response::from_string(format!(r#"{{"error": "Mock not found", "path": "{}"}}"#, mock_file.display()))
            .with_status_code(404)
            .with_header(content_type_header(b"application/json"))
    }
}

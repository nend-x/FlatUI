// Minimal HTTP server on 127.0.0.1:2290
// Listens for POST /toggle and toggles the launcher window.
//
// Endpoints:
//   POST /toggle  → toggle launcher visibility
//   GET  /status  → returns {"running":true}
//   GET  /        → 200 OK (health check)
//
// This is meant to be called from an AutoHotkey script that intercepts
// the Win key (since WH_KEYBOARD_LL on Win11 is unreliable for this).

use std::io::{Read, Write, BufRead, BufReader};
use std::net::TcpListener;
use tauri::{AppHandle, Emitter, Manager};

pub fn start_http_server(app: AppHandle) {
    std::thread::spawn(move || {
        let listener = match TcpListener::bind("127.0.0.1:2290") {
            Ok(l) => l,
            Err(e) => {
                log::error!("HTTP server bind failed on :2290 — {e}");
                return;
            }
        };
        log::info!("HTTP server listening on 127.0.0.1:2290");

        for stream in listener.incoming() {
            match stream {
                Ok(mut stream) => {
                    let app = app.clone();
                    std::thread::spawn(move || {
                        if let Err(e) = handle_request(&mut stream, &app) {
                            log::debug!("HTTP request error: {e}");
                        }
                    });
                }
                Err(e) => {
                    log::debug!("TCP accept error: {e}");
                }
            }
        }
    });
}

fn handle_request(
    stream: &mut std::net::TcpStream,
    app: &AppHandle,
) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    // Parse: "METHOD PATH HTTP/1.1"
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        write_response(stream, 400, "Bad Request")?;
        return Ok(());
    }
    let method = parts[0];
    let path = parts[1];

    // Read remaining headers (we don't care about them, but need to consume)
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line)?;
        if n == 0 || line.trim().is_empty() {
            break;
        }
        let lower = line.to_lowercase();
        if let Some(v) = lower.strip_prefix("content-length:") {
            content_length = v.trim().parse().unwrap_or(0);
        }
    }

    // Read body if any
    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        let _ = reader.read_exact(&mut body);
    }

    // CORS headers — allow any origin so AHK COM HTTP request works
    let cors = "Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: POST, GET, OPTIONS\r\nAccess-Control-Allow-Headers: *\r\n";

    match (method, path) {
        ("OPTIONS", _) => {
            write_response_full(stream, 204, "No Content", cors, "")?;
        }
        ("POST", "/toggle") => {
            log::info!("HTTP /toggle received");
            let app_clone = app.clone();
            std::thread::spawn(move || {
                toggle_launcher(&app_clone);
            });
            write_response_full(stream, 200, "OK", cors, "{\"ok\":true}")?;
        }
        ("GET", "/status") => {
            let launcher_visible = app
                .get_webview_window("launcher")
                .map(|w| w.is_visible().unwrap_or(false))
                .unwrap_or(false);
            let body = format!(
                "{{\"running\":true,\"launcher_visible\":{}}}",
                launcher_visible
            );
            write_response_full(stream, 200, "OK", cors, &body)?;
        }
        ("GET", "/") | ("GET", "") => {
            write_response_full(stream, 200, "OK", cors, "flatui")?;
        }
        _ => {
            write_response_full(stream, 404, "Not Found", cors, "")?;
        }
    }

    Ok(())
}

fn write_response(stream: &mut std::net::TcpStream, code: u16, status: &str) -> std::io::Result<()> {
    write_response_full(stream, code, status, "", "")
}

fn write_response_full(
    stream: &mut std::net::TcpStream,
    code: u16,
    status: &str,
    extra_headers: &str,
    body: &str,
) -> std::io::Result<()> {
    let response = format!(
        "HTTP/1.1 {code} {status}\r\n{extra_headers}Content-Type: application/json\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n{body}",
        len = body.len()
    );
    stream.write_all(response.as_bytes())?;
    stream.flush()?;
    Ok(())
}

fn toggle_launcher(app: &AppHandle) {
    log::info!("toggle_launcher (http): delegating to main implementation");
    crate::toggle_launcher_impl(app);
}

/// Show the run dialog (Win+R replacement).
/// Emits an event to the launcher frontend which shows a small input dialog.
fn show_run_dialog(app: &AppHandle) {
    if let Some(launcher) = app.get_webview_window("launcher") {
        if !launcher.is_visible().unwrap_or(false) {
            let _ = launcher.show();
            let _ = launcher.set_focus();
            let _ = launcher.set_always_on_top(true);
        }
        let _ = app.emit("launcher://show-run-dialog", ());
    } else {
        log::error!("show_run_dialog: launcher window not found");
    }
}

#[allow(dead_code)]
fn _unused_keep_show_run_dialog() {
    // show_run_dialog is no longer called from HTTP endpoint,
    // but kept as a useful utility if we need to trigger it from elsewhere
    // (e.g. a future Tauri command). Mark dead_code suppressed.
}

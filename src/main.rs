use std::{
    net::{IpAddr, SocketAddr},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use eframe::{App, NativeOptions, egui};
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use serde::Deserialize;
use tokio::fs::File;
use tokio::{net::TcpListener, sync::oneshot};
use tokio_util::io::ReaderStream;

fn main() -> Result<(), eframe::Error> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([520.0, 310.0])
        .with_min_inner_size([520.0, 310.0])
        .with_max_inner_size([520.0, 310.0])
        .with_resizable(false)
        .with_maximize_button(false);

    if let Ok(icon) = eframe::icon_data::from_png_bytes(include_bytes!("../logo.png")) {
        viewport = viewport.with_icon(icon);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "i-serve",
        options,
        Box::new(|_cc| Ok(Box::new(IServeApp::default()))),
    )
}

struct IServeApp {
    selected_folder: Option<PathBuf>,
    port: String,
    status: String,
    running: bool,
    server: Option<ServerHandle>,
    local_ips: Vec<IpAddr>,
    last_height: f32,
}

impl Default for IServeApp {
    fn default() -> Self {
        Self {
            selected_folder: None,
            port: String::new(),
            status: String::new(),
            running: false,
            server: None,
            local_ips: Vec::new(),
            last_height: 310.0,
        }
    }
}

impl App for IServeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.apply_modern_theme(ctx);
        self.update_window_size(ctx);

        if self.port.is_empty() {
            self.port = "8080".to_string();
        }
        if self.local_ips.is_empty() {
            self.local_ips = discover_local_ips();
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(egui::Color32::from_rgb(243, 247, 255))
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let painter = ui.painter();
                painter.circle_filled(
                    rect.left_top() + egui::vec2(120.0, 20.0),
                    220.0,
                    egui::Color32::from_rgba_unmultiplied(74, 125, 255, 32),
                );
                painter.circle_filled(
                    rect.right_bottom() - egui::vec2(80.0, 60.0),
                    180.0,
                    egui::Color32::from_rgba_unmultiplied(43, 184, 157, 26),
                );

                egui::Frame::default()
                    .fill(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 235))
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(12.0)
                    .inner_margin(egui::Margin::same(14))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("i-serve")
                                        .size(28.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(34, 48, 78)),
                                );
                                ui.label(
                                    egui::RichText::new("Modern Local File Web Server")
                                        .size(14.0)
                                        .strong()
                                        .color(egui::Color32::BLACK),
                                );
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if !self.status.is_empty() {
                                        let status_color = self.status_color();
                                        egui::Frame::default()
                                            .fill(status_color)
                                            .corner_radius(5.0)
                                            .inner_margin(egui::Margin::symmetric(10, 6))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    egui::RichText::new(self.status.clone())
                                                        .size(15.0)
                                                        .strong()
                                                        .color(egui::Color32::BLACK),
                                                );
                                            });
                                    }
                                },
                            );
                        });

                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new("Shared Folder")
                                .size(13.0)
                                .strong()
                                .color(egui::Color32::BLACK),
                        );
                        ui.horizontal(|ui| {
                            let folder_text = self
                                .selected_folder
                                .as_ref()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "No folder selected".to_string());
                            let mut folder_display = folder_text;
                            ui.add_sized(
                                [ui.available_width() - 130.0, 32.0],
                                egui::TextEdit::singleline(&mut folder_display)
                                    .interactive(false)
                                    .horizontal_align(egui::Align::Center)
                                    .hint_text("No folder selected"),
                            );
                            if ui
                                .add_sized([116.0, 32.0], egui::Button::new("Pick Folder"))
                                .clicked()
                            {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.selected_folder = Some(folder);
                                }
                            }
                        });

                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Port")
                                    .size(13.0)
                                    .strong()
                                    .color(egui::Color32::BLACK),
                            );
                            ui.add_sized(
                                [100.0, 32.0],
                                egui::TextEdit::singleline(&mut self.port)
                                    .horizontal_align(egui::Align::Center),
                            );
                        });

                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !self.running,
                                    egui::Button::new("Start Server")
                                        .min_size(egui::vec2(130.0, 34.0)),
                                )
                                .clicked()
                            {
                                self.start_server();
                            }

                            if ui
                                .add_enabled(
                                    self.running,
                                    egui::Button::new("Stop Server")
                                        .min_size(egui::vec2(130.0, 34.0)),
                                )
                                .clicked()
                            {
                                self.stop_server();
                            }
                        });

                        if self.running {
                            if let Ok(port) = self.port.parse::<u16>() {
                                ui.add_space(12.0);
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Access URLs")
                                        .size(13.0)
                                        .strong()
                                        .color(egui::Color32::BLACK),
                                );
                                for ip in &self.local_ips {
                                    ui.monospace(
                                        egui::RichText::new(format!("http://{}:{}/", ip, port))
                                            .strong()
                                            .color(egui::Color32::BLACK),
                                    );
                                }
                            }
                        }
                    });
            });
    }
}

impl IServeApp {
    fn update_window_size(&mut self, ctx: &egui::Context) {
        let target_height = if self.running { 410.0 } else { 310.0 };
        if (self.last_height - target_height).abs() > f32::EPSILON {
            let size = egui::vec2(520.0, target_height);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::MaxInnerSize(size));
            self.last_height = target_height;
        }
    }

    fn apply_modern_theme(&self, ctx: &egui::Context) {
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 7.0);
        style.override_font_id = Some(egui::FontId::proportional(16.0));
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = egui::Color32::from_rgb(243, 247, 255);
        style.visuals.override_text_color = Some(egui::Color32::BLACK);
        style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(239, 244, 255);
        style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(225, 234, 251);
        style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(208, 223, 248);
        style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(184, 206, 243);
        style.visuals.widgets.open.bg_fill = egui::Color32::from_rgb(198, 216, 246);
        style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;
        style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
        style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
        style.visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
        style.visuals.widgets.open.bg_stroke = egui::Stroke::NONE;
        style.visuals.widgets.inactive.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.hovered.fg_stroke.color = egui::Color32::BLACK;
        style.visuals.widgets.active.fg_stroke.color = egui::Color32::BLACK;
        ctx.set_style(style);
    }

    fn status_color(&self) -> egui::Color32 {
        if self.running {
            egui::Color32::from_rgb(30, 157, 115)
        } else if self.status.to_ascii_lowercase().contains("error")
            || self.status.to_ascii_lowercase().contains("failed")
            || self.status.to_ascii_lowercase().contains("invalid")
        {
            egui::Color32::from_rgb(201, 68, 93)
        } else {
            egui::Color32::from_rgb(95, 123, 179)
        }
    }

    fn start_server(&mut self) {
        let Some(folder) = self.selected_folder.clone() else {
            self.status = "Pick a folder first".to_string();
            return;
        };

        let Ok(port) = self.port.parse::<u16>() else {
            self.status = "Invalid port".to_string();
            return;
        };

        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        thread::spawn(move || {
            let panic_sender = ready_tx.clone();
            let run_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .map_err(|err| format!("Runtime error: {err}"))?;

                runtime.block_on(async move {
                    let state = AppState {
                        root: Arc::new(folder),
                    };

                    let app = Router::new()
                        .route("/", get(index_handler))
                        .route("/raw/{*path}", get(raw_handler))
                        .route("/logo.png", get(logo_handler))
                        .with_state(state);

                    let addr = SocketAddr::from(([0, 0, 0, 0], port));
                    let listener = TcpListener::bind(addr)
                        .await
                        .map_err(|err| format!("Bind error: {err}"))?;

                    let _ = ready_tx.send(Ok(()));

                    let server = axum::serve(listener, app).with_graceful_shutdown(async {
                        let _ = shutdown_rx.await;
                    });

                    if let Err(err) = server.await {
                        eprintln!("server error: {err}");
                    }

                    Ok::<(), String>(())
                })?;

                Ok::<(), String>(())
            }));

            match run_result {
                Ok(Ok(())) => {}
                Ok(Err(err)) => {
                    let _ = panic_sender.send(Err(err));
                }
                Err(payload) => {
                    let panic_msg = if let Some(s) = payload.downcast_ref::<&str>() {
                        *s
                    } else if let Some(s) = payload.downcast_ref::<String>() {
                        s.as_str()
                    } else {
                        "unknown panic"
                    };
                    let _ = panic_sender.send(Err(format!("Server panicked: {panic_msg}")));
                }
            }
        });

        match ready_rx.recv() {
            Ok(Ok(())) => {
                self.running = true;
                self.status = format!("Running on port {port}");
                self.server = Some(ServerHandle {
                    shutdown: Mutex::new(Some(shutdown_tx)),
                });
            }
            Ok(Err(err)) => {
                self.status = err;
            }
            Err(err) => {
                self.status = format!("Server startup channel failed: {err}");
            }
        }
    }

    fn stop_server(&mut self) {
        if let Some(handle) = self.server.take() {
            if let Ok(mut guard) = handle.shutdown.lock() {
                if let Some(shutdown) = guard.take() {
                    let _ = shutdown.send(());
                }
            }
        }

        self.running = false;
        self.status = "Server stopped".to_string();
    }
}

struct ServerHandle {
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
}

#[derive(Clone)]
struct AppState {
    root: Arc<PathBuf>,
}

#[derive(Deserialize)]
struct IndexQuery {
    q: Option<String>,
    dir: Option<String>,
}

async fn index_handler(State(state): State<AppState>, Query(query): Query<IndexQuery>) -> Response {
    let q = query.q.unwrap_or_default();
    let dir_param = query.dir.unwrap_or_default();
    let current_dir = match normalize_rel_dir(&dir_param) {
        Some(path) => path,
        None => return (StatusCode::BAD_REQUEST, "Invalid directory").into_response(),
    };
    let full_dir = state.root.join(&current_dir);

    let entries = match std::fs::read_dir(&full_dir) {
        Ok(read_dir) => {
            let mut items = Vec::new();
            for entry in read_dir.flatten() {
                if let Ok(meta) = entry.metadata() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !q.is_empty() && !name.to_lowercase().contains(&q.to_lowercase()) {
                        continue;
                    }
                    let relative_path = current_dir.join(&name);
                    items.push(FileEntry {
                        name,
                        rel_path: relative_path,
                        is_dir: meta.is_dir(),
                        size: if meta.is_file() {
                            Some(meta.len())
                        } else {
                            None
                        },
                    });
                }
            }
            items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
            items
        }
        Err(err) => {
            let body = format!("Failed to list directory: {err}");
            return (StatusCode::INTERNAL_SERVER_ERROR, body).into_response();
        }
    };

    let file_cards = entries
        .iter()
        .map(|f| {
            let badge = if f.is_dir {
                "<span class=\"badge bg-secondary\">folder</span>".to_string()
            } else {
                "<span class=\"badge bg-success\">file</span>".to_string()
            };

            let size = f
                .size
                .map(human_size)
                .map(|s| format!("<small class=\"text-muted\">{s}</small>"))
                .unwrap_or_default();

            let safe_path = encode_rel_path_for_route(&f.rel_path);
            let link = if f.is_dir {
                format!("/?dir={safe_path}")
            } else {
                format!("/raw/{safe_path}")
            };

            format!(
                r#"<div class="col-md-6 col-lg-4 file-item" data-name="{name_lower}">
                    <div class="card shadow-sm h-100 border-0">
                        <div class="card-body d-flex justify-content-between align-items-center">
                            <div>
                                <h6 class="card-title mb-1">{name}</h6>
                                {size}
                            </div>
                            <div class="text-end">
                                {badge}
                                <a class="btn btn-sm btn-outline-primary ms-2" href="{link}">Download</a>
                            </div>
                        </div>
                    </div>
                </div>"#,
                name = html_escape(&f.name),
                name_lower = f.name.to_lowercase(),
                size = size,
                badge = badge,
                link = link,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let parent_link = current_dir.parent().map(|p| {
        format!(
            r#"<a class="btn btn-outline-secondary" href="/?dir={}">Up one level</a>"#,
            utf8_percent_encode(&p.to_string_lossy(), NON_ALPHANUMERIC)
        )
    });

    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>i-serve</title>
  <link rel="icon" type="image/png" href="/logo.png" />
  <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.3/dist/css/bootstrap.min.css" rel="stylesheet">
  <style>
    body {{
      min-height: 100vh;
      background: linear-gradient(135deg, #f8fafc 0%, #eef2ff 100%);
      font-family: "Segoe UI", Tahoma, Geneva, Verdana, sans-serif;
    }}
    .shell {{
      max-width: 1100px;
      margin: 40px auto;
      padding: 24px;
      background: rgba(255,255,255,.8);
      border-radius: 18px;
      backdrop-filter: blur(6px);
    }}
    .title {{ letter-spacing: .2px; font-weight: 700; }}
    .brand-logo {{
      width: 36px;
      height: 36px;
      border-radius: 10px;
      object-fit: cover;
      box-shadow: 0 6px 18px rgba(22, 31, 58, .18);
    }}
  </style>
</head>
<body>
  <main class="shell">
    <div class="d-flex flex-wrap justify-content-between align-items-center mb-3 gap-3">
      <div class="d-flex align-items-center gap-3">
        <img src="/logo.png" alt="i-serve logo" class="brand-logo" />
        <div>
          <h2 class="title mb-1">i-serve file browser</h2>
          <p class="text-muted mb-0">Sharing local files on your network</p>
          <p class="mb-0"><small class="text-muted">Current folder: /{current_dir}</small></p>
        </div>
      </div>
      <form method="get" class="d-flex gap-2" role="search">
        <input type="hidden" name="dir" value="{dir_query}" />
        <input id="search" name="q" value="{query}" class="form-control" placeholder="Filter files..." />
        <button class="btn btn-primary" type="submit">Filter</button>
      </form>
    </div>
    {parent_link}
    <div class="mb-3"></div>

    <div class="row g-3" id="file-grid">
      {file_cards}
    </div>
  </main>

  <script>
    const input = document.getElementById('search');
    const cards = Array.from(document.querySelectorAll('.file-item'));
    input?.addEventListener('input', (e) => {{
      const value = e.target.value.toLowerCase();
      for (const card of cards) {{
        const name = card.dataset.name || '';
        card.style.display = name.includes(value) ? '' : 'none';
      }}
    }});
  </script>
</body>
</html>
"#,
        query = html_escape(&q),
        dir_query = html_escape(&current_dir.to_string_lossy()),
        current_dir = html_escape(&current_dir.to_string_lossy()),
        parent_link = parent_link.unwrap_or_default(),
        file_cards = file_cards,
    );

    Html(html).into_response()
}

async fn logo_handler() -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("image/png"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("public, max-age=3600"),
    );

    (
        StatusCode::OK,
        headers,
        include_bytes!("../logo.png").as_ref(),
    )
        .into_response()
}

async fn raw_handler(State(state): State<AppState>, AxumPath(path): AxumPath<String>) -> Response {
    const STREAM_THRESHOLD_BYTES: u64 = 1024 * 1024 * 1024; // 1 GB

    let relative = PathBuf::from(path);
    if !is_safe_relative_path(&relative) {
        return (StatusCode::BAD_REQUEST, "Invalid path").into_response();
    }

    let full_path = state.root.join(&relative);
    let metadata = match tokio::fs::metadata(&full_path).await {
        Ok(meta) if meta.is_file() => meta,
        Ok(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
        Err(_) => return (StatusCode::NOT_FOUND, "File not found").into_response(),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/octet-stream"),
    );

    if let Some(name) = full_path.file_name().and_then(|n| n.to_str()) {
        let content_disposition = format!("attachment; filename=\"{}\"", sanitize_header(name));
        if let Ok(value) = header::HeaderValue::from_str(&content_disposition) {
            headers.insert(header::CONTENT_DISPOSITION, value);
        }
    }

    if let Ok(content_len) = header::HeaderValue::from_str(&metadata.len().to_string()) {
        headers.insert(header::CONTENT_LENGTH, content_len);
    }

    if metadata.len() > STREAM_THRESHOLD_BYTES {
        match File::open(&full_path).await {
            Ok(file) => {
                let stream = ReaderStream::new(file);
                let body = Body::from_stream(stream);
                (StatusCode::OK, headers, body).into_response()
            }
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Open error: {err}"),
            )
                .into_response(),
        }
    } else {
        match tokio::fs::read(&full_path).await {
            Ok(bytes) => (StatusCode::OK, headers, bytes).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Read error: {err}"),
            )
                .into_response(),
        }
    }
}

fn discover_local_ips() -> Vec<IpAddr> {
    let mut ips = vec![IpAddr::from([127, 0, 0, 1])];
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            if iface.is_loopback() {
                continue;
            }
            let ip = iface.ip();
            if !ips.contains(&ip) {
                ips.push(ip);
            }
        }
    }
    ips
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|c| {
            !matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

fn human_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let b = bytes as f64;
    if b >= GB {
        format!("{:.2} GB", b / GB)
    } else if b >= MB {
        format!("{:.2} MB", b / MB)
    } else if b >= KB {
        format!("{:.2} KB", b / KB)
    } else {
        format!("{} B", bytes)
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn sanitize_header(input: &str) -> String {
    input.chars().filter(|c| *c != '\\' && *c != '"').collect()
}

fn normalize_rel_dir(input: &str) -> Option<PathBuf> {
    if input.is_empty() {
        return Some(PathBuf::new());
    }
    let path = PathBuf::from(input);
    if is_safe_relative_path(&path) {
        Some(path)
    } else {
        None
    }
}

fn encode_rel_path_for_route(path: &Path) -> String {
    path.components()
        .filter_map(|comp| match comp {
            Component::Normal(s) => {
                Some(utf8_percent_encode(&s.to_string_lossy(), NON_ALPHANUMERIC).to_string())
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

struct FileEntry {
    name: String,
    rel_path: PathBuf,
    is_dir: bool,
    size: Option<u64>,
}

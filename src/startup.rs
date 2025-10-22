use std::{
    io::ErrorKind,
    net::TcpListener,
    path::{Component, Path, PathBuf},
};

use actix_files::NamedFile;
use actix_web::{
    HttpRequest, HttpResponse, Result as ActixResult,
    dev::Server,
    error::{ErrorInternalServerError, ErrorNotFound},
    web,
};
use anyhow::{Context, anyhow};
use notify::{
    RecommendedWatcher, RecursiveMode, Watcher,
    event::{EventKind, ModifyKind, RenameMode},
    recommended_watcher,
};
use tokio::fs;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{Duration, sleep};

use crate::{
    config::{self, DevServerConfig},
    internal_scope::build_internal_scope,
};

#[derive(Clone)]
pub struct AppState {
    pub base_dir: PathBuf,
    pub broadcaster: broadcast::Sender<LiveMessage>,
    pub diff_mode: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LiveMessage {
    Reload,
    Diff {
        path: String,
        resource: DiffResource,
    },
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DiffResource {
    Html,
    Css,
}

pub struct Application {
    server: Server,
    port: u16,
    _watcher: RecommendedWatcher,
    state: AppState,
}

impl Application {
    pub async fn build(config: &DevServerConfig) -> anyhow::Result<Self> {
        let allow_fallback = config.port == config::DEFAULT_PORT;
        let (listener, port) = bind_listener(config.port, allow_fallback)?;

        if allow_fallback && port != config.port {
            println!(
                "[web-dev-server] port {} in use, switched to {}",
                config.port, port
            );
        }

        let base_dir = resolve_base_dir(&config.base_dir)
            .with_context(|| format!("failed to resolve base directory {}", config.base_dir))?;

        let (broadcaster, _) = broadcast::channel(64);

        let state = AppState {
            base_dir: base_dir.clone(),
            broadcaster: broadcaster.clone(),
            diff_mode: config.diff_mode,
        };

        let (watcher, notify_rx) = create_watcher(&state)?;
        spawn_watcher_loop(state.clone(), notify_rx);

        let server = run(listener, state.clone()).await?;

        Ok(Self {
            server,
            port,
            _watcher: watcher,
            state,
        })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn base_dir(&self) -> &Path {
        &self.state.base_dir
    }

    pub fn diff_mode(&self) -> bool {
        self.state.diff_mode
    }

    pub fn primary_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub async fn run_until_stopped(self) -> std::io::Result<()> {
        self.server.await
    }
}

fn bind_listener(preferred_port: u16, allow_fallback: bool) -> anyhow::Result<(TcpListener, u16)> {
    let mut port = preferred_port;

    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok((listener, port)),
            Err(error) if allow_fallback && error.kind() == ErrorKind::AddrInUse => {
                if port == u16::MAX {
                    return Err(anyhow!(
                        "failed to find an available port starting at {}",
                        preferred_port
                    ));
                }
                port = port.checked_add(1).ok_or_else(|| {
                    anyhow!(
                        "failed to find an available port starting at {}",
                        preferred_port
                    )
                })?;
            }
            Err(error) => {
                return Err(anyhow::Error::from(error)
                    .context(format!("failed to bind to 127.0.0.1:{port}")));
            }
        }
    }
}

async fn run(listener: TcpListener, state: AppState) -> anyhow::Result<Server> {
    let shared_state = web::Data::new(state);

    let server = actix_web::HttpServer::new(move || {
        actix_web::App::new()
            .app_data(shared_state.clone())
            .service(build_internal_scope())
            .service(web::resource("/{tail:.*}").route(web::to(serve_file)))
    })
    .listen(listener)?
    .run();

    Ok(server)
}

fn resolve_base_dir(base_dir: &str) -> anyhow::Result<PathBuf> {
    let path = PathBuf::from(base_dir);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };

    let canonical = absolute.canonicalize()?;
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        anyhow::bail!("base directory must be a directory")
    }
}

fn create_watcher(
    state: &AppState,
) -> anyhow::Result<(
    RecommendedWatcher,
    mpsc::UnboundedReceiver<notify::Result<notify::Event>>,
)> {
    let (tx, rx) = mpsc::unbounded_channel();
    let root = state.base_dir.clone();

    let mut watcher = recommended_watcher(move |res| {
        let _ = tx.send(res);
    })?;

    watcher.watch(&root, RecursiveMode::Recursive)?;

    Ok((watcher, rx))
}

fn spawn_watcher_loop(
    state: AppState,
    mut rx: mpsc::UnboundedReceiver<notify::Result<notify::Event>>,
) {
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                Ok(event) => {
                    let state_for_event = state.clone();
                    tokio::spawn(async move {
                        sleep(Duration::from_millis(120)).await;
                        handle_fs_event(state_for_event, event);
                    });
                }
                Err(error) => {
                    eprintln!("[web-dev-server] watcher error: {error}");
                    let _ = state.broadcaster.send(LiveMessage::Reload);
                }
            }
        }
    });
}

fn handle_fs_event(state: AppState, event: notify::Event) {
    if !state.diff_mode {
        let _ = state.broadcaster.send(LiveMessage::Reload);
        return;
    }

    if event.need_rescan() {
        let _ = state.broadcaster.send(LiveMessage::Reload);
        return;
    }

    let kind = event.kind;

    if should_ignore_event(&kind) {
        return;
    }

    let mut diff_messages = Vec::new();

    for path in event.paths {
        if let Some(message) = classify_path(&state, &path) {
            diff_messages.push(message);
        }
    }

    if !diff_messages.is_empty() {
        if allows_diff(&kind) {
            for message in diff_messages {
                let _ = state.broadcaster.send(message);
            }
            return;
        } else {
            let _ = state.broadcaster.send(LiveMessage::Reload);
            return;
        }
    }

    if should_reload_when_no_diff(&kind) {
        let _ = state.broadcaster.send(LiveMessage::Reload);
    }
}

fn should_ignore_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Name(RenameMode::From))
    )
}

fn allows_diff(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Metadata(_))
            | EventKind::Modify(ModifyKind::Any)
            | EventKind::Modify(ModifyKind::Name(
                RenameMode::To | RenameMode::Both | RenameMode::Any | RenameMode::Other
            ))
    )
}

fn should_reload_when_no_diff(kind: &EventKind) -> bool {
    match kind {
        EventKind::Remove(_) | EventKind::Other | EventKind::Any => true,
        EventKind::Modify(ModifyKind::Name(mode)) => !matches!(mode, RenameMode::From),
        EventKind::Modify(ModifyKind::Other) => true,
        _ => false,
    }
}

fn classify_path(state: &AppState, path: &Path) -> Option<LiveMessage> {
    let normalized = normalize_event_path(&state.base_dir, path)?;
    let ext = normalized.extension()?.to_str()?.to_ascii_lowercase();
    let resource = match ext.as_str() {
        "html" | "htm" => DiffResource::Html,
        "css" => DiffResource::Css,
        _ => return None,
    };

    let web_path = to_web_path(&state.base_dir, &normalized, &resource)?;

    Some(LiveMessage::Diff {
        path: web_path,
        resource,
    })
}

fn normalize_event_path(base_dir: &Path, path: &Path) -> Option<PathBuf> {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return Some(canonical);
    }

    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };

    if let Ok(canonical) = std::fs::canonicalize(&resolved) {
        Some(canonical)
    } else {
        Some(resolved)
    }
}

fn to_web_path(base_dir: &Path, path: &Path, resource: &DiffResource) -> Option<String> {
    let relative = path.strip_prefix(base_dir).ok()?;
    let mut rel_str = relative.to_string_lossy().replace('\\', "/");
    if rel_str.is_empty() {
        return Some(String::from("/"));
    }

    rel_str = rel_str.trim_start_matches('/').to_owned();

    match resource {
        DiffResource::Html => {
            if rel_str.ends_with("index.html") {
                let prefix = rel_str.trim_end_matches("index.html");
                if prefix.is_empty() {
                    Some(String::from("/"))
                } else {
                    let trimmed = prefix.trim_end_matches('/');
                    let mut path = format!("/{}", trimmed);
                    if !path.ends_with('/') {
                        path.push('/');
                    }
                    Some(path)
                }
            } else if rel_str.ends_with("index.htm") {
                let prefix = rel_str.trim_end_matches("index.htm");
                if prefix.is_empty() {
                    Some(String::from("/"))
                } else {
                    let trimmed = prefix.trim_end_matches('/');
                    let mut path = format!("/{}", trimmed);
                    if !path.ends_with('/') {
                        path.push('/');
                    }
                    Some(path)
                }
            } else {
                Some(format!("/{}", rel_str))
            }
        }
        DiffResource::Css => Some(format!("/{}", rel_str)),
    }
}

async fn serve_file(
    req: HttpRequest,
    tail: web::Path<String>,
    state: web::Data<AppState>,
) -> ActixResult<HttpResponse> {
    let target = locate_file(&state.base_dir, tail.as_str())
        .await
        .map_err(|_| ErrorNotFound("Not Found"))?;

    if is_html(&target) {
        let raw = fs::read_to_string(&target)
            .await
            .map_err(ErrorInternalServerError)?;
        let injected =
            inject_live_client(&raw, state.diff_mode).map_err(ErrorInternalServerError)?;

        Ok(HttpResponse::Ok()
            .append_header(("Cache-Control", "no-cache, no-store, must-revalidate"))
            .content_type("text/html; charset=utf-8")
            .body(injected))
    } else {
        let file = NamedFile::open_async(&target)
            .await
            .map_err(|_| ErrorNotFound("Not Found"))?;

        Ok(file.into_response(&req))
    }
}

async fn locate_file(base_dir: &Path, tail: &str) -> anyhow::Result<PathBuf> {
    let mut full_path = sanitize_path(base_dir, tail)?;

    if let Ok(metadata) = fs::metadata(&full_path).await {
        if metadata.is_dir() {
            let index_html = full_path.join("index.html");
            if fs::metadata(&index_html).await.is_ok() {
                full_path = index_html;
            } else {
                anyhow::bail!("directory has no index.html");
            }
        }
        Ok(full_path)
    } else {
        anyhow::bail!("file not found")
    }
}

fn sanitize_path(base_dir: &Path, tail: &str) -> anyhow::Result<PathBuf> {
    let trimmed = tail.trim_start_matches('/');
    let mut target = PathBuf::from(base_dir);

    if trimmed.is_empty() {
        target.push("index.html");
        return Ok(target);
    }

    let mut has_component = false;

    for component in Path::new(trimmed).components() {
        match component {
            Component::Normal(part) => {
                target.push(part);
                has_component = true;
            }
            Component::CurDir => {}
            _ => anyhow::bail!("invalid path"),
        }
    }

    if !has_component && tail.ends_with('/') {
        target.push("index.html");
    }

    Ok(target)
}

fn is_html(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| matches!(ext.to_ascii_lowercase().as_str(), "html" | "htm"))
        .unwrap_or(false)
}

fn inject_live_client(original: &str, diff_mode: bool) -> anyhow::Result<String> {
    if original.contains("__web_dev_server_client") {
        return Ok(original.to_string());
    }

    let config = serde_json::json!({
        "wsPath": "/_live/ws",
        "diffMode": diff_mode,
    });

    let snippet = format!(
        r#"<script id="__web_dev_server_config">window.__WEB_DEV_SERVER_CONFIG__ = {};</script><script id="__web_dev_server_client" defer src="/_live/script.js"></script>"#,
        serde_json::to_string(&config)?
    );

    if let Some(idx) = original.rfind("</head>") {
        let mut result = String::with_capacity(original.len() + snippet.len() + 2);
        result.push_str(&original[..idx]);
        result.push('\n');
        result.push_str(&snippet);
        result.push('\n');
        result.push_str(&original[idx..]);
        Ok(result)
    } else {
        let mut result = original.to_string();
        if !result.ends_with('\n') {
            result.push('\n');
        }
        result.push_str(&snippet);
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::event::{AccessKind, DataChange, ModifyKind, RemoveKind, RenameMode};

    #[test]
    fn diff_message_serializes_resource_lowercase() {
        let message = LiveMessage::Diff {
            path: "/".into(),
            resource: DiffResource::Html,
        };
        let json = serde_json::to_string(&message).unwrap();
        assert!(
            json.contains(r#""resource":"html""#),
            "serialized json was {json}"
        );
    }

    #[test]
    fn access_events_are_ignored_for_diff_mode() {
        assert!(should_ignore_event(&EventKind::Access(AccessKind::Read)));
    }

    #[test]
    fn rename_from_events_are_ignored() {
        let event = EventKind::Modify(ModifyKind::Name(RenameMode::From));
        assert!(should_ignore_event(&event));
    }

    #[test]
    fn modify_data_events_allow_diff() {
        let event = EventKind::Modify(ModifyKind::Data(DataChange::Any));
        assert!(allows_diff(&event));
    }

    #[test]
    fn metadata_events_allow_diff() {
        let event = EventKind::Modify(ModifyKind::Metadata(notify::event::MetadataKind::WriteTime));
        assert!(allows_diff(&event));
    }

    #[test]
    fn remove_events_force_reload_when_no_diff() {
        let event = EventKind::Remove(RemoveKind::File);
        assert!(should_reload_when_no_diff(&event));
    }

    #[test]
    fn relative_paths_are_classified_within_base_dir() {
        let base_dir =
            std::env::temp_dir().join(format!("web_dev_server_test_{}", std::process::id()));
        std::fs::create_dir_all(&base_dir).unwrap();
        let canonical = std::fs::canonicalize(&base_dir).unwrap();
        let (tx, _) = broadcast::channel(1);
        let state = AppState {
            base_dir: canonical,
            broadcaster: tx,
            diff_mode: true,
        };

        let message = classify_path(&state, Path::new("index.html"))
            .expect("expected diff message for html file");

        if let LiveMessage::Diff { path, resource } = message {
            assert_eq!(path, "/");
            assert!(matches!(resource, DiffResource::Html));
        } else {
            panic!("expected diff message");
        }
    }
}

use std::ffi::{CStr, CString};
use std::fs;
use std::mem;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::prelude::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;

use awa_core::auth::{AuthEngine, AuthReport};
use awa_core::config::Config;
use awa_ipc::framing::{DEFAULT_SOCKET_PATH, read_json, write_json};
use awa_ipc::messages::{Request, Response};

const CONN_TIMEOUT: Duration = Duration::from_secs(30);
static PASSWD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Parser, Debug)]
#[command(name = "awa-daemon", version, about = "awa authentication daemon")]
struct Args {
    /// path to config.toml
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// unix socket path
    #[arg(long, default_value = DEFAULT_SOCKET_PATH)]
    socket: PathBuf,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let cfg = match &args.config {
        Some(path) => {
            Config::load(path).with_context(|| format!("load config {}", path.display()))?
        }
        None => Config::discover().context("discover config")?.1,
    };

    tracing::info!("loading auth engine");
    let engine = Arc::new(Mutex::new(
        AuthEngine::new(cfg).context("load auth engine")?,
    ));
    let listener = bind_socket(&args.socket)?;
    tracing::info!("listening on {}", args.socket.display());

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                tracing::warn!("accept failed: {}", e);
                continue;
            }
        };

        if let Err(e) = stream.set_read_timeout(Some(CONN_TIMEOUT)) {
            tracing::warn!("set read timeout failed: {}", e);
        }
        if let Err(e) = stream.set_write_timeout(Some(CONN_TIMEOUT)) {
            tracing::warn!("set write timeout failed: {}", e);
        }

        let engine = Arc::clone(&engine);
        std::thread::spawn(move || {
            if let Err(e) = handle_client(&mut stream, engine) {
                tracing::warn!("client failed: {:?}", e);
            }
        });
    }

    Ok(())
}

fn bind_socket(path: &Path) -> Result<UnixListener> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create socket dir {}", parent.display()))?;
    }

    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_socket() => {
            fs::remove_file(path)
                .with_context(|| format!("remove stale socket {}", path.display()))?;
        }
        Ok(_) => anyhow::bail!("socket path exists and is not a socket: {}", path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!("stat socket {}", path.display())),
    }

    let listener =
        UnixListener::bind(path).with_context(|| format!("bind socket {}", path.display()))?;
    // socket is world-rw; the actual auth boundary is so_peercred in handle_client.
    // any local user can connect, but daemon checks peer uid before honoring auth requests.
    fs::set_permissions(path, fs::Permissions::from_mode(0o666))
        .with_context(|| format!("chmod socket {}", path.display()))?;
    Ok(listener)
}

fn handle_client(stream: &mut UnixStream, engine: Arc<Mutex<AuthEngine>>) -> Result<()> {
    let request: Request = read_json(stream).context("read request")?;
    let uid = peer_uid(stream);
    let mut should_exit = false;
    let response = match request {
        Request::Authenticate { username } => {
            tracing::info!("auth request user={} peer_uid={:?}", username, uid);
            if peer_can_authenticate(stream, &username) {
                let mut guard = engine.lock().unwrap_or_else(|e| e.into_inner());
                authenticate(&mut guard, &username)
            } else {
                Response::Error {
                    message: "peer uid cannot authenticate requested user".to_string(),
                }
            }
        }
        Request::Status => {
            let guard = engine.lock().unwrap_or_else(|e| e.into_inner());
            Response::StatusResponse {
                models_loaded: true,
                camera_ready: true,
                has_ir: guard.has_ir(),
            }
        }
        Request::Shutdown if uid == Some(0) => {
            should_exit = true;
            Response::ShutdownAck
        }
        Request::Shutdown => Response::Error {
            message: "shutdown requires root peer".to_string(),
        },
        Request::Enroll { .. } => Response::Error {
            message: "enroll is not implemented in daemon".to_string(),
        },
    };

    write_json(stream, &response).context("write response")?;

    if should_exit {
        tracing::info!("shutdown requested by root peer; exiting");
        std::process::exit(0);
    }

    Ok(())
}

fn authenticate(engine: &mut AuthEngine, username: &str) -> Response {
    match engine.authenticate(username) {
        Ok(report) if report.pass => {
            tracing::info!(
                "auth success user={} attempts={} face_score={:.4} similarity={:.4} liveness={:.4} elapsed={:.2}s",
                report.user,
                report.attempts,
                report.face_score.unwrap_or(0.0),
                report.similarity.unwrap_or(0.0),
                report.liveness_score.unwrap_or(0.0),
                report.elapsed.as_secs_f32(),
            );
            Response::AuthSuccess {
                similarity: report.similarity.unwrap_or(0.0),
                liveness_score: report.liveness_score.unwrap_or(0.0),
            }
        }
        Ok(report) => {
            tracing::info!(
                "auth failure user={} reason={} attempts={} face_score={:.4} similarity={:.4} liveness={:.4} elapsed={:.2}s",
                report.user,
                report.reason.as_deref().unwrap_or("authentication failed"),
                report.attempts,
                report.face_score.unwrap_or(0.0),
                report.similarity.unwrap_or(0.0),
                report.liveness_score.unwrap_or(0.0),
                report.elapsed.as_secs_f32(),
            );
            auth_failure(report)
        }
        Err(e) => Response::Error {
            message: e.to_string(),
        },
    }
}

fn auth_failure(report: AuthReport) -> Response {
    Response::AuthFailure {
        reason: report
            .reason
            .unwrap_or_else(|| "authentication failed".to_string()),
        best_similarity: report.similarity.unwrap_or(0.0),
    }
}

fn peer_can_authenticate(stream: &UnixStream, username: &str) -> bool {
    match peer_uid(stream) {
        Some(0) => true,
        Some(uid) => uid_for_user(username) == Some(uid),
        None => false,
    }
}

fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let mut cred = mem::MaybeUninit::<libc::ucred>::uninit();
    let mut len = mem::size_of::<libc::ucred>() as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            cred.as_mut_ptr().cast(),
            &mut len,
        )
    };
    if rc == 0 {
        Some(unsafe { cred.assume_init().uid })
    } else {
        None
    }
}

fn uid_for_user(username: &str) -> Option<u32> {
    let username = CString::new(username).ok()?;
    let _guard = PASSWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    unsafe {
        let passwd = libc::getpwnam(username.as_ptr());
        if passwd.is_null() {
            return None;
        }
        let name = CStr::from_ptr((*passwd).pw_name);
        if name.to_bytes() == username.as_bytes() {
            Some((*passwd).pw_uid)
        } else {
            None
        }
    }
}

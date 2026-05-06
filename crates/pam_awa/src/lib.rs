use std::ffi::CStr;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use awa_ipc::framing::{DEFAULT_SOCKET_PATH, read_json, write_json};
use awa_ipc::messages::{Request, Response};
use pamsm::{Pam, PamError, PamFlags, PamLibExt, PamServiceModule, pam_module};

struct PamAwa;

impl PamServiceModule for PamAwa {
    fn authenticate(pamh: Pam, _flags: PamFlags, args: Vec<String>) -> PamError {
        if should_skip_face(&pamh, &args) {
            return PamError::IGNORE;
        }

        let user = match pam_user(&pamh) {
            Ok(Some(user)) => user,
            Ok(None) => return PamError::USER_UNKNOWN,
            Err(e) => return e,
        };
        let socket = socket_path(&args);

        match request_auth(&socket, &user) {
            Ok(true) => PamError::SUCCESS,
            Ok(false) => PamError::AUTH_ERR,
            Err(_) => PamError::AUTHINFO_UNAVAIL,
        }
    }
}

pam_module!(PamAwa);

fn should_skip_face(pamh: &Pam, args: &[String]) -> bool {
    if args.iter().any(|arg| arg == "always") {
        return false;
    }
    matches!(pamh.get_cached_authtok(), Ok(Some(token)) if !token.to_bytes().is_empty())
}

fn pam_user(pamh: &Pam) -> Result<Option<String>, PamError> {
    if let Some(user) = pam_cstr_to_string(pamh.get_ruser()?) {
        return Ok(Some(user));
    }
    Ok(pam_cstr_to_string(pamh.get_user(None)?))
}

fn pam_cstr_to_string(value: Option<&CStr>) -> Option<String> {
    let value = value?;
    if value.to_bytes().is_empty() {
        None
    } else {
        Some(value.to_string_lossy().into_owned())
    }
}

fn socket_path(args: &[String]) -> PathBuf {
    args.iter()
        .find_map(|arg| arg.strip_prefix("socket=").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH))
}

fn request_auth(socket: &Path, user: &str) -> Result<bool, awa_ipc::framing::IpcError> {
    let mut stream = UnixStream::connect(socket)?;
    let timeout = Some(Duration::from_secs(15));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;

    let request = Request::Authenticate {
        username: user.to_string(),
    };
    write_json(&mut stream, &request)?;

    match read_json::<_, Response>(&mut stream)? {
        Response::AuthSuccess { .. } => Ok(true),
        Response::AuthFailure { .. } => Ok(false),
        Response::Error { .. } => Ok(false),
        _ => Ok(false),
    }
}

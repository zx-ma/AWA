use std::ffi::{CStr, CString};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use awa_core::config::Config;

#[derive(Parser, Debug)]
#[command(name = "awa", version, about = "linux face authentication")]
struct Cli {
    /// path to config.toml (overrides default search)
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// register a face for a user     
    Enroll {
        /// username (defaults to $USER)                                            
        #[arg(short, long)]
        user: Option<String>,
        /// label name (e.g. "primary", "with_glasses")
        #[arg(short, long, default_value = "primary")]
        label: String,
        /// number of samples to capture
        #[arg(short = 'n', long, default_value_t = 3)]
        samples: usize,
    },
    /// authenticate a user against stored face                                     
    Auth {
        /// username (defaults to $USER)                                            
        #[arg(short, long)]
        user: Option<String>,
    },
    /// run pipeline on a static image (no camera)
    Test {
        /// image file path            
        #[arg(short, long)]
        image: PathBuf,
    },
    /// list available cameras         
    Device {
        #[command(subcommand)]
        action: DeviceAction,
    },
    /// print current configuration                                                 
    ConfigShow,
    /// install binaries, pam module, and systemd service
    Install {
        /// release artifact directory
        #[arg(long)]
        build_dir: Option<PathBuf>,
        /// install prefix for awa and awa-daemon
        #[arg(long, default_value = "/usr/local")]
        prefix: PathBuf,
        /// pam module directory
        #[arg(long)]
        pam_dir: Option<PathBuf>,
        /// systemd system unit directory
        #[arg(long, default_value = "/etc/systemd/system")]
        systemd_dir: PathBuf,
        /// daemon unix socket path
        #[arg(long, default_value = awa_ipc::framing::DEFAULT_SOCKET_PATH)]
        socket: PathBuf,
        /// install files without enabling or starting the service
        #[arg(long)]
        no_enable: bool,
    },
    /// enable or disable pam services
    Pam {
        #[command(subcommand)]
        action: PamAction,
    },
}

#[derive(Subcommand, Debug)]
enum DeviceAction {
    /// list video devices and their formats      
    List,
}

#[derive(Subcommand, Debug)]
enum PamAction {
    /// add pam_awa to a service
    Enable {
        /// pam service name, for example sudo or hyprlock
        service: String,
        /// daemon unix socket path
        #[arg(long, default_value = awa_ipc::framing::DEFAULT_SOCKET_PATH)]
        socket: PathBuf,
    },
    /// restore the service backup or remove pam_awa
    Disable {
        /// pam service name, for example sudo or hyprlock
        service: String,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let config = cli.config.clone();

    match cli.command {
        Command::Enroll {
            user,
            label,
            samples,
        } => {
            let cfg = load_config(config.as_ref())?;
            let user = resolve_user(user)?;
            run_enroll(&cfg, &user, &label, samples)?;
        }
        Command::Auth { user } => {
            let cfg = load_config(config.as_ref())?;
            let user = resolve_user(user)?;
            run_auth(&cfg, &user)?;
        }
        Command::Test { image } => {
            println!("[stub] test image={}", image.display());
        }
        Command::Device { action } => match action {
            DeviceAction::List => println!("[stub] device list"),
        },
        Command::ConfigShow => {
            let cfg = load_config(config.as_ref())?;
            let pretty = toml::to_string_pretty(&cfg).context("serialize config")?;
            println!("{pretty}");
        }
        Command::Install {
            build_dir,
            prefix,
            pam_dir,
            systemd_dir,
            socket,
            no_enable,
        } => {
            run_install(
                config.as_ref(),
                build_dir,
                &prefix,
                pam_dir.as_ref(),
                &systemd_dir,
                &socket,
                no_enable,
            )?;
        }
        Command::Pam { action } => match action {
            PamAction::Enable { service, socket } => {
                run_pam_enable(&service, &socket)?;
            }
            PamAction::Disable { service } => {
                run_pam_disable(&service)?;
            }
        },
    }

    Ok(())
}

fn load_config(path: Option<&PathBuf>) -> Result<Config> {
    match path {
        Some(p) => Config::load(p).with_context(|| format!("load config {}", p.display())),
        None => Ok(Config::discover().context("discover config")?.1),
    }
}

fn resolve_config_path(path: Option<&PathBuf>) -> Result<PathBuf> {
    let path = match path {
        Some(p) => p.clone(),
        None => match Config::discover() {
            Ok((path, _)) => path,
            Err(e) => sudo_user_config_path()
                .filter(|path| path.exists())
                .ok_or_else(|| anyhow::anyhow!(e))?,
        },
    };
    path.canonicalize()
        .with_context(|| format!("canonicalize config {}", path.display()))
}

fn sudo_user_config_path() -> Option<PathBuf> {
    let user = std::env::var("SUDO_USER").ok()?;
    user_home(&user).map(|home| home.join(".config/awa/config.toml"))
}

fn user_home(user: &str) -> Option<PathBuf> {
    let user = CString::new(user).ok()?;
    unsafe {
        let passwd = libc::getpwnam(user.as_ptr());
        if passwd.is_null() {
            return None;
        }
        let home = CStr::from_ptr((*passwd).pw_dir);
        Some(PathBuf::from(home.to_string_lossy().as_ref()))
    }
}

fn run_install(
    config: Option<&PathBuf>,
    build_dir: Option<PathBuf>,
    prefix: &Path,
    pam_dir: Option<&PathBuf>,
    systemd_dir: &Path,
    socket: &Path,
    no_enable: bool,
) -> Result<()> {
    let config_path = resolve_config_path(config)?;
    let build_dir = match build_dir {
        Some(path) => path,
        None => std::env::current_exe()
            .context("resolve current executable")?
            .parent()
            .context("current executable has no parent directory")?
            .to_path_buf(),
    };
    let pam_dir = match pam_dir {
        Some(path) => path.clone(),
        None => discover_pam_dir()?,
    };

    let bin_dir = prefix.join("bin");
    let awa_src = build_dir.join("awa");
    let daemon_src = build_dir.join("awa-daemon");
    let pam_src = build_dir.join("libpam_awa.so");
    let awa_dst = bin_dir.join("awa");
    let daemon_dst = bin_dir.join("awa-daemon");
    let pam_dst = pam_dir.join("pam_awa.so");
    let unit_dst = systemd_dir.join("awa-daemon.service");

    install_file(&awa_src, &awa_dst, 0o755)?;
    install_file(&daemon_src, &daemon_dst, 0o755)?;
    install_file(&pam_src, &pam_dst, 0o755)?;

    fs::create_dir_all(systemd_dir)
        .with_context(|| format!("create systemd dir {}", systemd_dir.display()))?;
    fs::write(
        &unit_dst,
        systemd_unit(&daemon_dst, &config_path, socket).as_bytes(),
    )
    .with_context(|| format!("write {}", unit_dst.display()))?;
    fs::set_permissions(&unit_dst, fs::Permissions::from_mode(0o644))
        .with_context(|| format!("chmod {}", unit_dst.display()))?;

    println!("installed {}", awa_dst.display());
    println!("installed {}", daemon_dst.display());
    println!("installed {}", pam_dst.display());
    println!("installed {}", unit_dst.display());

    if no_enable {
        println!("service not enabled; run `sudo systemctl enable --now awa-daemon.service`");
    } else {
        run_systemctl(&["daemon-reload"])?;
        run_systemctl(&["enable", "--now", "awa-daemon.service"])?;
        println!("enabled and started awa-daemon.service");
    }

    println!();
    println!("pam line for sudo or hyprlock:");
    println!("auth sufficient pam_awa.so");
    Ok(())
}

fn discover_pam_dir() -> Result<PathBuf> {
    for path in [
        "/usr/lib/security",
        "/usr/lib64/security",
        "/lib/security",
        "/lib/x86_64-linux-gnu/security",
    ] {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Ok(path);
        }
    }
    anyhow::bail!("could not find pam module directory; pass --pam-dir")
}

fn install_file(src: &Path, dst: &Path, mode: u32) -> Result<()> {
    if !src.exists() {
        anyhow::bail!(
            "missing {}; build release artifacts first or pass --build-dir",
            src.display()
        );
    }
    if paths_are_same(src, dst) {
        fs::set_permissions(dst, fs::Permissions::from_mode(mode))
            .with_context(|| format!("chmod {}", dst.display()))?;
        return Ok(());
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::copy(src, dst).with_context(|| format!("copy {} to {}", src.display(), dst.display()))?;
    fs::set_permissions(dst, fs::Permissions::from_mode(mode))
        .with_context(|| format!("chmod {}", dst.display()))?;
    Ok(())
}

fn paths_are_same(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn systemd_unit(daemon: &Path, config: &Path, socket: &Path) -> String {
    format!(
        "[Unit]\n\
Description=Awa face authentication daemon\n\
After=local-fs.target\n\
\n\
[Service]\n\
Type=simple\n\
Environment=RUST_LOG=awa_daemon=info,awa_core=info,ort=warn\n\
RuntimeDirectory=awa\n\
RuntimeDirectoryMode=0755\n\
ExecStart={} --config {} --socket {}\n\
Restart=on-failure\n\
RestartSec=2\n\
\n\
[Install]\n\
WantedBy=multi-user.target\n",
        systemd_arg(daemon),
        systemd_arg(config),
        systemd_arg(socket),
    )
}

fn systemd_arg(path: &Path) -> String {
    let value = path.display().to_string();
    if value.chars().any(char::is_whitespace) {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        value
    }
}

fn run_systemctl(args: &[&str]) -> Result<()> {
    let status = ProcessCommand::new("systemctl")
        .args(args)
        .status()
        .with_context(|| format!("run systemctl {}", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("systemctl {} failed with {}", args.join(" "), status);
    }
    Ok(())
}

fn run_pam_enable(service: &str, socket: &Path) -> Result<()> {
    let path = pam_service_path(service)?;
    let backup = pam_backup_path(service)?;
    let original = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;

    if original.lines().any(|line| line.contains("pam_awa.so")) {
        println!("{} already contains pam_awa.so", path.display());
        return Ok(());
    }

    if !backup.exists() {
        fs::copy(&path, &backup)
            .with_context(|| format!("backup {} to {}", path.display(), backup.display()))?;
        println!("backed up {}", backup.display());
    }

    let line = pam_auth_line(socket);
    let mut output = String::new();
    let mut inserted = false;
    for existing in original.lines() {
        if !inserted && existing.trim_start().starts_with("auth") {
            output.push_str(&line);
            output.push('\n');
            inserted = true;
        }
        output.push_str(existing);
        output.push('\n');
    }
    if !inserted {
        output.push_str(&line);
        output.push('\n');
    }

    write_pam_service(&path, &output)?;
    println!("enabled pam_awa for {}", service);
    Ok(())
}

fn run_pam_disable(service: &str) -> Result<()> {
    let path = pam_service_path(service)?;
    let backup = pam_backup_path(service)?;
    if backup.exists() {
        fs::copy(&backup, &path)
            .with_context(|| format!("restore {} to {}", backup.display(), path.display()))?;
        println!("restored {}", path.display());
        return Ok(());
    }

    let original = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let output = original
        .lines()
        .filter(|line| !line.contains("pam_awa.so"))
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    write_pam_service(&path, &output)?;
    println!("removed pam_awa from {}", path.display());
    Ok(())
}

fn pam_service_path(service: &str) -> Result<PathBuf> {
    if service.is_empty() || service.contains('/') {
        anyhow::bail!("invalid pam service name: {service}");
    }
    Ok(PathBuf::from("/etc/pam.d").join(service))
}

fn pam_backup_path(service: &str) -> Result<PathBuf> {
    Ok(PathBuf::from("/etc/pam.d").join(format!("{service}.awa.bak")))
}

fn pam_auth_line(socket: &Path) -> String {
    if socket == Path::new(awa_ipc::framing::DEFAULT_SOCKET_PATH) {
        "auth    sufficient  pam_awa.so".to_string()
    } else {
        format!("auth    sufficient  pam_awa.so socket={}", socket.display())
    }
}

fn write_pam_service(path: &Path, contents: &str) -> Result<()> {
    let metadata = fs::metadata(path).with_context(|| format!("stat {}", path.display()))?;
    let tmp = path.with_file_name(format!(
        ".{}.tmp.{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("pam-service"),
        std::process::id()
    ));
    fs::write(&tmp, contents).with_context(|| format!("write {}", tmp.display()))?;
    fs::set_permissions(&tmp, metadata.permissions())
        .with_context(|| format!("chmod {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

fn resolve_user(arg: Option<String>) -> Result<String> {
    if let Some(u) = arg {
        return Ok(u);
    }
    if let Ok(u) = std::env::var("PAM_RUSER")
        && !u.is_empty()
    {
        return Ok(u);
    }
    if let Ok(u) = std::env::var("PAM_USER")
        && !u.is_empty()
    {
        return Ok(u);
    }
    std::env::var("USER").context("$USER not set; pass --user explicitly")
}

fn run_enroll(cfg: &Config, user: &str, label: &str, num_samples: usize) -> Result<()> {
    use std::thread::sleep;
    use std::time::Duration;

    use awa_core::camera::{CameraConfig, CameraSet};
    use awa_core::enrollment::store::EnrollmentStore;
    use awa_core::pipeline::align::align_face;
    use awa_core::pipeline::arcface::extract_embedding;
    use awa_core::pipeline::scrfd::detect;
    use awa_core::pipeline::{ModelPaths as PipelineModelPaths, Pipeline};

    println!("loading models...");
    let model_paths = PipelineModelPaths {
        scrfd: &cfg.models.scrfd,
        arcface: &cfg.models.arcface,
        minifas: &cfg.models.minifas,
    };
    let mut pipe = Pipeline::load(&model_paths).context("load pipeline")?;

    println!("opening cameras...");
    let cam_cfg = CameraConfig {
        rgb_path: &cfg.camera.rgb_device,
        rgb_width: cfg.camera.rgb_width,
        rgb_height: cfg.camera.rgb_height,
        ir_path: cfg.camera.ir_device.as_deref(),
        ir_width: cfg.camera.ir_width,
        ir_height: cfg.camera.ir_height,
    };
    let cameras = CameraSet::open(&cam_cfg).context("open cameras")?;

    let store = EnrollmentStore::new(&cfg.store.base_dir);

    println!("enrolling user={user} label={label} samples={num_samples}");
    println!("look at the camera. capture starts in 2 seconds...");
    sleep(Duration::from_secs(2));

    let mut collected = 0;
    let mut attempt = 0;
    let max_attempts = num_samples * 3;

    while collected < num_samples && attempt < max_attempts {
        attempt += 1;
        println!("  [{collected}/{num_samples}] capturing...");
        let frame = cameras.capture().context("capture frame")?;

        let faces = detect(&mut pipe.scrfd, &frame.rgb).context("detect")?;
        let face = match faces.first() {
            Some(f) if f.score > 0.6 => f,
            _ => {
                println!("    no clear face detected, retry");
                sleep(Duration::from_millis(500));
                continue;
            }
        };

        let aligned = align_face(&frame.rgb, &face.keypoints);
        let embedding = extract_embedding(&mut pipe.arcface, &aligned).context("embed")?;

        store
            .add_sample(user, label, embedding, "arcface_w600k_r50")
            .context("save sample")?;

        collected += 1;
        println!("    sample {collected} saved (score={:.3})", face.score);
        sleep(Duration::from_millis(800));
    }

    if collected < num_samples {
        anyhow::bail!("only collected {collected}/{num_samples} samples");
    }

    println!("\nenrollment complete: {collected} samples for {user}/{label}");
    println!("data: {}", store.base_dir().display());
    Ok(())
}

fn run_auth(cfg: &Config, user: &str) -> Result<()> {
    use awa_core::auth::AuthEngine;

    let mut engine = AuthEngine::new(cfg.clone()).context("load auth engine")?;

    println!("authenticating {user}...");
    let report = engine.authenticate(user).context("authenticate")?;
    let similarity = report.similarity.unwrap_or(0.0);
    let liveness = report.liveness_score.unwrap_or(0.0);
    let face_score = report.face_score.unwrap_or(0.0);

    println!();
    println!("user:        {}", report.user);
    println!("face score:  {:.3}", face_score);
    println!(
        "similarity:  {:.4}  (threshold {:.2}) {}",
        similarity,
        cfg.auth.threshold,
        if report.pass_match { "PASS" } else { "FAIL" }
    );
    println!(
        "liveness:    {:.4}  (threshold {:.2}) {}",
        liveness,
        cfg.auth.liveness_threshold,
        if report.pass_liveness { "PASS" } else { "FAIL" }
    );
    if let Some(reason) = &report.reason {
        println!("reason:      {reason}");
    }
    println!("attempts:    {}", report.attempts);
    println!("elapsed:     {:.2}s", report.elapsed.as_secs_f32());
    println!();
    println!(
        "result:      {}",
        if report.pass {
            "AUTHENTICATED"
        } else {
            "DENIED"
        }
    );

    if !report.pass {
        std::process::exit(1);
    }
    Ok(())
}

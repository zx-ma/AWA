use std::path::PathBuf;

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
}

#[derive(Subcommand, Debug)]
enum DeviceAction {
    /// list video devices and their formats      
    List,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let config_path = cli.config.clone();
    let cfg = match config_path {
        Some(p) => Config::load(&p).with_context(|| format!("load config {}", p.display()))?,
        None => Config::discover().context("discover config")?.1,
    };

    match cli.command {
        Command::Enroll {
            user,
            label,
            samples,
        } => {
            let user = resolve_user(user)?;
            run_enroll(&cfg, &user, &label, samples)?;
        }
        Command::Auth { user } => {
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
            let pretty = toml::to_string_pretty(&cfg).context("serialize config")?;
            println!("{pretty}");
        }
    }

    Ok(())
}

fn resolve_user(arg: Option<String>) -> Result<String> {
    if let Some(u) = arg {
        return Ok(u);
    }
    if let Ok(u) = std::env::var("PAM_RUSER") {
        if !u.is_empty() {
            return Ok(u);
        }
    }
    if let Ok(u) = std::env::var("PAM_USER") {
        if !u.is_empty() {
            return Ok(u);
        }
    }
    std::env::var("USER").context("$USER not set; pass --user explicitly")
}

fn run_enroll(cfg: &Config, user: &str, label: &str, num_samples: usize) -> Result<()> {
    use std::sync::Arc;
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
    let pipeline = Pipeline::load(&model_paths).context("load pipeline")?;
    let mut pipe = Arc::try_unwrap(pipeline)
        .ok()
        .context("pipeline single ref")?;

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

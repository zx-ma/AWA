# awa

**A**wa **W**inks **A**lice — linux face authentication

the name doubles as a face kaomoji `(´・ω・`)` and a recursive acronym where the IR camera winks to authenticate the user

## status

works as a standalone CLI. PAM integration not done yet.

| stage | status |
|---|---|
| face detection (scrfd 10g) | done |
| face alignment (umeyama 112x112) | done |
| face embedding (arcface 512d) | done |
| rgb liveness (facenox) | done |
| dual camera capture (rgb + ir) | done |
| ir liveness check | not yet |
| enrollment storage (multi-sample) | done |
| `awa enroll` / `awa auth` cli | done |
| pam module | not yet |

## quick start

### prerequisites

- linux with v4l2
- a webcam (rgb required, ir optional)
- rust toolchain

### setup

```bash
git clone <this repo> awa && cd awa

# download models (see "models" section below)

# install config
mkdir -p ~/.config/awa
cp config.toml.example ~/.config/awa/config.toml
$EDITOR ~/.config/awa/config.toml   # adjust device paths and model paths

# build
cargo build --release
```

### use

```bash
# enroll your face (3 samples by default)
./target/release/awa enroll

# try authenticating
RUST_LOG=warn ./target/release/awa auth
```

`auth` exits 0 on pass, 1 on fail.

## models

models are not bundled. download manually:

```bash
# face detection (insightface antelopev2, non-commercial research only)
wget https://github.com/deepinsight/insightface/releases/download/v0.7/antelopev2.zip
unzip antelopev2.zip && cp antelopev2/scrfd_10g_bnkps.onnx models/

# face recognition (insightface buffalo_l, non-commercial research only)
wget https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_l.zip
unzip buffalo_l.zip && cp buffalo_l/w600k_r50.onnx models/arcface_w600k_r50.onnx

# liveness detection (apache-2.0)
wget https://github.com/facenox/face-antispoof-onnx/releases/download/v1.0.0/best_model.onnx -O models/minifas_v2.onnx
```

## camera setup

find your devices:

```bash
v4l2-ctl --list-devices
v4l2-ctl -d /dev/video0 --list-formats-ext   # repeat for each device
```

rgb cameras typically advertise `MJPG` or `YUYV`. ir cameras advertise `GREY`. fill the paths into `~/.config/awa/config.toml`. if no ir camera, comment out `ir_device`.

## architecture

```
crates/
  awa-core/    pipeline, camera, enrollment, config
  awa-cli/     `awa` binary
  awa-ipc/     pam ↔ daemon message types (unused for now)
  awa-daemon/  daemon stub (unused for now)
  pam_awa/     pam cdylib stub (unused for now)
```

`awa-cli` reads `~/.config/awa/config.toml` and talks directly to `awa-core`. when pam integration lands, the daemon will own the camera and pipeline, and the cli/pam-module will become thin clients over a unix socket.

## security notes

- liveness is rgb-only right now. screen replay attacks are detected probabilistically by the facenox model but not deterministically. ir-based liveness will close this gap.
- enrollment data is stored as 512-dim float vectors in `~/.local/share/awa/enrollments/<user>.json` with mode `0600`. these are biometric templates — protect them.
- not yet hardened for production use. no encrypted storage, no tpm binding, no anti-replay nonce.

## license

copyright (c) 2026 zhexuan ma

GNU General Public License v3.0 or later. see [LICENSE](./LICENSE).

**model licenses are separate.** the insightface models (`scrfd_10g_bnkps.onnx`, `arcface_w600k_r50.onnx`) are non-commercial research only. the facenox liveness model is apache-2.0.

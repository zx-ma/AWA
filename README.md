# AWA

**A**wa **W**inks **A**lice - linux face authentication

the name doubles as a face kaomoji `(´・ω・`)` and a recursive acronym where the IR camera winks to authenticate the user

## status

works as cli + daemon + PAM module now. still early. do not treat this as production hardened auth yet.

| stage | status |
|---|---|
| face detection (scrfd 10g) | done |
| face alignment (umeyama 112x112) | done |
| face embedding (arcface 512d) | done |
| rgb liveness (facenox) | done |
| multi-frame auth retry | done |
| dual camera capture (rgb + ir) | done |
| ir liveness check | not yet |
| enrollment storage (multi-sample) | done |
| `awa enroll` / `awa auth` cli | done |
| daemon over unix socket | done |
| thin pam module | done |
| installer command | basic |

## quick start

### prerequisites

- linux with v4l2
- a webcam (rgb required, ir optional)
- rust toolchain
- systemd if you want the daemon service
- `pamtester` is useful for testing, but optional

### setup

```bash
git clone <this repo> awa && cd awa

# download models (see "models" section below)

mkdir -p ~/.config/awa
cp config.toml.example ~/.config/awa/config.toml
$EDITOR ~/.config/awa/config.toml

cargo build --release
```

check auth before touching PAM:

```bash
target/release/awa --config ~/.config/awa/config.toml enroll --user "$USER"
RUST_LOG=awa_core=debug,ort=warn target/release/awa --config ~/.config/awa/config.toml auth --user "$USER"
```

install binaries, PAM module, and daemon service:

```bash
sudo target/release/awa --config ~/.config/awa/config.toml install
systemctl status awa-daemon.service
```

this installs roughly:

```text
/usr/local/bin/awa
/usr/local/bin/awa-daemon
/usr/lib/security/pam_awa.so
/etc/systemd/system/awa-daemon.service
```

PAM paths can differ by distro. pass `--pam-dir` if auto-detect gets it wrong.

## enable pam

start with hyprlock, not sudo.

```bash
sudo awa pam enable hyprlock
```

for hyprlock, empty enter has to reach PAM:

```conf
general {
    ignore_empty_input = false
}
```

lock the screen, press enter without typing a password. face auth should run. if face fails, type password.

sudo is riskier. keep a root shell open while testing:

```bash
sudo -i
```

then in another terminal:

```bash
sudo awa pam enable sudo
sudo -k
sudo whoami
```

rollback:

```bash
sudo awa pam disable sudo
sudo awa pam disable hyprlock
sudo systemctl disable --now awa-daemon.service
```

## manual pam test

if you want to test without touching sudo or hyprlock:

```bash
sudo tee /etc/pam.d/awa-cascade >/dev/null <<'EOF'
auth    sufficient  pam_awa.so
auth    required    pam_unix.so
account required    pam_permit.so
session required    pam_permit.so
EOF

pamtester awa-cascade "$USER" authenticate
```

face pass should authenticate directly. face fail should fall back to password.

## models

models are not bundled. download manually:

```bash
mkdir -p models

# face detection (insightface antelopev2, non-commercial research only)
wget https://github.com/deepinsight/insightface/releases/download/v0.7/antelopev2.zip
unzip antelopev2.zip
cp antelopev2/scrfd_10g_bnkps.onnx models/

# face recognition (insightface buffalo_l, non-commercial research only)
wget https://github.com/deepinsight/insightface/releases/download/v0.7/buffalo_l.zip
unzip buffalo_l.zip
cp buffalo_l/w600k_r50.onnx models/arcface_w600k_r50.onnx

# liveness detection (apache-2.0)
wget https://github.com/facenox/face-antispoof-onnx/releases/download/v1.0.0/best_model.onnx -O models/minifas_v2.onnx
```

then make sure `~/.config/awa/config.toml` points to these files.

## camera setup

find your devices:

```bash
v4l2-ctl --list-devices
v4l2-ctl -d /dev/video0 --list-formats-ext
```

rgb cameras typically advertise `MJPG` or `YUYV`. ir cameras advertise `GREY`. fill the paths into `~/.config/awa/config.toml`. if no ir camera, comment out `ir_device`.

## tuning

`awa auth` prints similarity and liveness.

similarity checks "is this enrolled user". liveness checks "does this look like a live face". they are separate.

if liveness is close but flaky, tune:

```toml
[auth]
liveness_threshold = 0.85
max_samples = 5
```

`max_samples` is auth retry count now. more samples is slower but less flaky.

## architecture

```text
crates/
  awa-core/    pipeline, camera, enrollment, auth engine, config
  awa-cli/     awa binary, enroll/auth/install/pam commands
  awa-ipc/     request/response messages + unix socket framing
  awa-daemon/  loads models + camera, serves auth requests
  pam_awa/     thin PAM cdylib, talks to daemon only
```

important bit:

```text
pam_awa.so
    -> unix socket
awa-daemon
    -> camera + onnx + enrollment
```

PAM does not load ONNX/runtime/camera code into sudo or hyprlock. that was the wrong shape.

## security notes

- liveness is rgb-only right now. screen replay attacks are detected probabilistically by the facenox model, not deterministically.
- enrollment data is stored as 512-dim float vectors in `~/.local/share/awa/enrollments/<user>.json` with mode `0600`. these are biometric templates. protect them.
- daemon socket checks peer uid. root can request any user; normal users can only request themselves.
- still early. no encrypted storage, no tpm binding, no anti-replay nonce.
- be careful with sudo PAM. keep a root shell open when changing it.

## license

copyright (c) 2026 Zhexuan Ma

GNU General Public License v3.0 or later. see [LICENSE](./LICENSE).

**model licenses are separate.** the insightface models (`scrfd_10g_bnkps.onnx`, `arcface_w600k_r50.onnx`) are non-commercial research only. the facenox liveness model is apache-2.0.

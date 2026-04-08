# awa

**A**wa **W**inks **A**lice — linux face authentication via PAM

the name doubles as a face kaomoji `(´・ω・`)` and a recursive acronym where the IR camera winks to authenticate the user

## models

models are not included and must be downloaded manually

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

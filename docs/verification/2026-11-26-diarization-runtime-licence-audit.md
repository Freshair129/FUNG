---
version: "0.1.0b"
created_at: "2026-11-26"
last_update: "2026-11-26"
status: "draft"
superseded_by: null
attributes:
  domain: "local-first-audio-ai"
  doc_type: "verification"
  scope: "Licence audit of every package in scripts/diarization-runtime-requirements.txt, for the redistribution question in the in-app diarization enable flow"
  language: "Thai-first"
---

# Licence audit — diarization runtime (98 แพ็กเกจ)

Parent: [`2026-11-26-diarization-in-app-enable-flow.md`](../specs/2026-11-26-diarization-in-app-enable-flow.md) §8 ข้อ 1 · Issue #137

> เอกสารนี้เป็นการรวบรวมข้อเท็จจริงทางเทคนิค ไม่ใช่ความเห็นทางกฎหมาย
> ข้อที่ระบุว่า "ต้องตัดสินใจ" ควรให้เจ้าของโปรเจกต์ (หรือที่ปรึกษากฎหมาย) ยืนยัน

## 1. วิธีตรวจ

- รายชื่อและเวอร์ชันจาก `scripts/diarization-runtime-requirements.txt` (CPU variant) — 98 แพ็กเกจ
- ดึง metadata จาก PyPI JSON API ของเวอร์ชันที่ตรึงไว้ (`license_expression`, `license`, classifiers)
- แพ็กเกจที่ metadata ว่างหรือเป็นข้อความยาว: อ่านไฟล์ licence จริงใน wheel (`cp311-win_amd64` หรือ `py3-none-any`)
  ด้วย HTTP range request — ไม่ได้ติดตั้งหรือรันโค้ดใดๆ
- wheel ที่มี native code (torch, numpy, scipy, pillow, grpcio, scikit-learn, torchcodec, matplotlib, hf_xet):
  ตรวจรายการไฟล์ licence และ DLL ที่ฝังมา
- ตรวจเมื่อ 2026-11-26

## 2. สรุป

| ผล | จำนวน | หมายเหตุ |
| --- | --- | --- |
| Permissive (MIT, BSD, Apache-2.0, PSF, ISC, 0BSD ฯลฯ) | 95 | แจกต่อได้ ต้องเก็บไฟล์ licence/notice ไว้ด้วย |
| Weak copyleft ระดับไฟล์ (MPL-2.0: `certifi`, `tqdm`, บางส่วนของ `grpcio`) | 3 | แจกได้ถ้าไม่แก้ไฟล์ ต้องเก็บ notice และบอกที่มาของ source |
| **ต้องตัดสินใจ** | 2 ประเด็น | ดู §3 |
| GPL/LGPL ที่ลิงก์เข้ากับโค้ดเรา | 0 | — |
| ไม่ทราบ licence | 0 | — |

ไม่พบแพ็กเกจที่ห้ามแจกต่อ แต่มีสองประเด็นที่ต้องตัดสินใจก่อนให้ FUNG เป็นผู้แจก archive เอง

## 3. ประเด็นที่ต้องตัดสินใจ

### 3.1 `libiomp5md.dll` ใน torch — เงื่อนไขแบบ proprietary ของ Intel (สำคัญที่สุด)

- wheel `torch-2.14.0-cp311-cp311-win_amd64.whl` มี `torch/lib/libiomp5md.dll` (Intel OpenMP runtime)
- ใน wheel **ไม่มีไฟล์ notice ของ DLL นี้** (ตรวจทั้ง `dist-info/licenses/` และ License-File ใน METADATA)
- แพ็กเกจ `intel-openmp` บน PyPI ระบุ "Intel End User License Agreement for Developer Tools" (Other/Proprietary)
- EULA ประเภทนี้ปกติอนุญาตให้แจก "redistributables" ได้แบบมีเงื่อนไข (แจกพร้อมแอปเท่านั้น, ห้าม reverse engineer
  ฯลฯ) — **ยังไม่ได้ยืนยันว่า DLL ใน torch มาจากแพ็กเกจนี้และอยู่ใต้ EULA ฉบับไหน**

### 3.2 GCC runtime ใน OpenBLAS ของ numpy/scipy — GPL-3.0 พร้อม runtime exception

- `numpy.libs/libscipy_openblas64_*.dll` และ `scipy.libs/libscipy_openblas*.dll` รวม libgfortran ไว้ข้างใน
- licence: `GPL-3.0-or-later WITH GCC-exception-3.1` (ระบุใน `LICENSE.txt` ของทั้งสอง wheel)
- exception ทำให้โค้ดของเราไม่ต้องเป็น GPL แต่การแจก binary ของ runtime เองยังอยู่ใต้ GPLv3
  → ผู้แจกต้องบอกแหล่ง source ที่ตรงกัน (numpy/scipy ทำโดยใส่ลิงก์ไว้ใน `LICENSE.txt`)

### 3.3 ข้อบังคับเล็กๆ ที่ต้องทำถ้าแจกเอง

- Pillow: FreeType License ต้องให้เครดิตในเอกสารของผลิตภัณฑ์
- MPL-2.0 (`certifi`, `tqdm`, `grpcio`): ห้ามแก้ไฟล์ และต้องเก็บ notice
- DLL ของ Microsoft VC++ runtime (`msvcp140.dll`, `vcomp140.dll` ใน numpy/scikit-learn): แจกได้ตามเงื่อนไข redistributable ของ Visual Studio
- matplotlib: ฟอนต์ DejaVu/STIX มี licence แยก (อยู่ใน wheel แล้ว)
- ปัจจุบัน `stage_diarization_runtime.ps1` เก็บแค่ licence ของ pyannote.audio และ PyTorch — ถ้าแจก archive ต้องรวม
  `*.dist-info/licenses/` ของทุกแพ็กเกจ

## 4. ข้อเสนอ: ให้ผู้ใช้ดาวน์โหลด wheel จาก PyPI โดยตรง แทนให้ FUNG แจก archive

ข้อค้นพบนี้ทำให้มีทางเลือกที่ดีกว่า §4 ของ spec:

- lockfile มี SHA-256 ของ wheel ทุกตัวอยู่แล้ว แอปดาวน์โหลด wheel จาก `files.pythonhosted.org` ตรวจ hash
  แล้วแตก zip ลงโฟลเดอร์ add-on ได้เลย (การติดตั้ง wheel คือการแตกไฟล์และวางตามตำแหน่ง ไม่มีการรันสคริปต์)
- ผลคือ **FUNG ไม่ได้เป็นผู้แจกต่อ** — ผู้ใช้ได้ไฟล์จากแหล่งเดียวกับที่ `pip install` ใช้ ภายใต้เงื่อนไขของผู้แจกเดิม
  ประเด็น §3.1–3.3 จึงไม่ใช่ภาระของ FUNG ในฐานะผู้แจก
- ความปลอดภัยเท่าเดิม: hash ทุกไฟล์ถูกตรึงใน lockfile ที่ review แล้ว
- ข้อยกเว้น: `antlr4-python3-runtime` (BSD-3-Clause) และ `docopt` (MIT) มีแต่ source archive บน PyPI
  ต้อง build เป็น wheel เองสองตัวนี้ — ทั้งคู่เป็น pure Python และ permissive จึงให้ FUNG แจกเองได้โดยไม่มีปัญหา
- ไม่ต้องมี CI job สำหรับ build/host archive ขนาดใหญ่

ถ้าเลือกทางนี้ ควรแก้ spec §4 และ open questions ข้อ 1, 3, 4 ตาม

## 5. รายการทั้งหมด

ช่อง Licence มาจาก `license_expression` ของ PyPI ก่อน, ถัดมาคือช่อง `license`, แล้วจึง classifier;
แถวที่มีวงเล็บหรือระบุส่วนที่ฝังมา อ่านจากไฟล์ licence ใน wheel โดยตรง
| # | Package | Version | Licence | Note |
|---|---|---|---|---|
| 1 | `aiohappyeyeballs` | 2.7.1 | PSF-2.0 |  |
| 2 | `aiohttp` | 3.14.3 | Apache-2.0 AND MIT |  |
| 3 | `aiosignal` | 1.4.0 | Apache 2.0 |  |
| 4 | `alembic` | 1.20.0 | MIT |  |
| 5 | `antlr4-python3-runtime` | 4.9.3 | BSD-3-Clause |  |
| 6 | `anyio` | 4.15.1 | MIT |  |
| 7 | `asteroid_filterbanks` | 0.4.0 | MIT |  |
| 8 | `attrs` | 26.1.0 | MIT |  |
| 9 | `certifi` | 2026.7.22 | MPL-2.0 | MPL-2.0: แจกได้ ไม่แก้ไฟล์ ต้องเก็บ notice |
| 10 | `charset_normalizer` | 3.5.2 | MIT |  |
| 11 | `click` | 8.5.0 | BSD-3-Clause |  |
| 12 | `cloudpickle` | 3.1.2 | BSD-3-Clause |  |
| 13 | `colorama` | 0.4.6 | BSD-3-Clause |  |
| 14 | `colorlog` | 6.12.0 | MIT License |  |
| 15 | `contourpy` | 1.3.3 | BSD-3-Clause |  |
| 16 | `cycler` | 0.12.1 | BSD-3-Clause |  |
| 17 | `docopt` | 0.6.2 | MIT |  |
| 18 | `einops` | 0.8.2 | MIT |  |
| 19 | `filelock` | 4.0.7 | MIT |  |
| 20 | `fonttools` | 4.66.1 | MIT |  |
| 21 | `frozenlist` | 1.8.0 | Apache-2.0 |  |
| 22 | `fsspec` | 2026.9.0 | BSD-3-Clause |  |
| 23 | `googleapis_common_protos` | 1.75.5 | Apache-2.0 |  |
| 24 | `grpcio` | 1.84.0 | Apache-2.0 (+ bundled BSD, MPL-2.0 parts) | มีส่วน MPL-2.0: แจกได้ ไม่แก้ไฟล์ ต้องเก็บ notice |
| 25 | `h11` | 0.16.0 | MIT |  |
| 26 | `hf_xet` | 1.6.0 | Apache-2.0 |  |
| 27 | `httpcore2` | 2.13.1 | BSD-3-Clause |  |
| 28 | `httpx2` | 2.13.1 | BSD-3-Clause |  |
| 29 | `huggingface_hub` | 2.0.0 | Apache-2.0 |  |
| 30 | `idna` | 3.20 | BSD-3-Clause |  |
| 31 | `jinja2` | 3.1.6 | BSD-3-Clause |  |
| 32 | `joblib` | 1.6.0 | BSD-3-Clause |  |
| 33 | `julius` | 0.2.8 | MIT License |  |
| 34 | `kiwisolver` | 1.5.1 | BSD-3-Clause |  |
| 35 | `lightning` | 2.6.6 | Apache-2.0 |  |
| 36 | `lightning_utilities` | 0.15.3 | Apache-2.0 |  |
| 37 | `mako` | 1.4.3 | MIT |  |
| 38 | `markdown_it_py` | 4.2.0 | MIT |  |
| 39 | `markupsafe` | 3.0.3 | BSD-3-Clause |  |
| 40 | `matplotlib` | 3.11.2 | PSF-based matplotlib licence (+ DejaVu/STIX font licences) |  |
| 41 | `mdurl` | 0.1.2 | MIT |  |
| 42 | `mpmath` | 1.3.0 | BSD-3-Clause |  |
| 43 | `multidict` | 6.9.1 | Apache License 2.0 |  |
| 44 | `narwhals` | 2.26.0 | MIT |  |
| 45 | `networkx` | 3.6.1 | BSD-3-Clause |  |
| 46 | `numpy` | 2.4.6 | BSD-3-Clause AND 0BSD AND MIT AND Zlib AND CC0-1.0 (+ bundled OpenBLAS / GCC runtime as scipy) | เหมือน scipy |
| 47 | `opentelemetry_api` | 1.45.0 | Apache-2.0 |  |
| 48 | `opentelemetry_exporter_http_transport` | 0.66b0 | Apache-2.0 |  |
| 49 | `opentelemetry_exporter_otlp` | 1.45.0 | Apache-2.0 |  |
| 50 | `opentelemetry_exporter_otlp_common` | 0.66b0 | Apache-2.0 |  |
| 51 | `opentelemetry_exporter_otlp_proto_common` | 1.45.0 | Apache-2.0 |  |
| 52 | `opentelemetry_exporter_otlp_proto_grpc` | 1.45.0 | Apache-2.0 |  |
| 53 | `opentelemetry_exporter_otlp_proto_http` | 1.45.0 | Apache-2.0 |  |
| 54 | `opentelemetry_proto` | 1.45.0 | Apache-2.0 |  |
| 55 | `opentelemetry_sdk` | 1.45.0 | Apache-2.0 |  |
| 56 | `opentelemetry_semantic_conventions` | 0.66b0 | Apache-2.0 |  |
| 57 | `optuna` | 5.0.0 | MIT |  |
| 58 | `packaging` | 26.3 | Apache-2.0 OR BSD-2-Clause |  |
| 59 | `pandas` | 3.0.6 | BSD-3-Clause |  |
| 60 | `pillow` | 12.3.0 | MIT-CMU (+ bundled FreeType FTL, libjpeg-turbo, libpng, zlib, HarfBuzz, liblzma 0BSD) | FreeType FTL ต้องให้เครดิตในเอกสาร |
| 61 | `primePy` | 1.3 | MIT |  |
| 62 | `propcache` | 0.5.4 | Apache-2.0 |  |
| 63 | `protobuf` | 7.36.2 | 3-Clause BSD License |  |
| 64 | `pyannote_audio` | 4.0.7 | MIT (+ Apache-2.0 for vendored WeSpeaker code) |  |
| 65 | `pyannote_core` | 6.0.1 | MIT |  |
| 66 | `pyannote_database` | 6.1.1 | MIT |  |
| 67 | `pyannote_metrics` | 4.1 | MIT |  |
| 68 | `pyannote_pipeline` | 4.0.0 | MIT |  |
| 69 | `pyannoteai_sdk` | 0.4.0 | MIT |  |
| 70 | `pygments` | 2.21.0 | BSD-2-Clause |  |
| 71 | `pyparsing` | 3.3.3 | MIT |  |
| 72 | `python_dateutil` | 2.9.0.post0 | Apache-2.0 OR BSD-3-Clause |  |
| 73 | `pytorch_lightning` | 2.6.6 | Apache-2.0 |  |
| 74 | `pytorch_metric_learning` | 2.9.0 | MIT |  |
| 75 | `pyyaml` | 6.0.3 | MIT |  |
| 76 | `requests` | 2.34.2 | Apache-2.0 |  |
| 77 | `rich` | 15.0.0 | MIT |  |
| 78 | `safetensors` | 0.8.0 | Apache-2.0 |  |
| 79 | `scikit_learn` | 1.9.1 | BSD-3-Clause (+ MSVC runtime DLLs) |  |
| 80 | `scipy` | 1.17.1 | BSD-3-Clause (+ bundled OpenBLAS: BSD-3-Clause, GCC runtime: GPL-3.0+ WITH GCC-exception-3.1) | GCC runtime เป็น GPL-3.0 + runtime exception: แจก binary ได้ แต่ต้องชี้แหล่ง source |
| 81 | `setuptools` | 84.0.0 | MIT |  |
| 82 | `six` | 1.17.0 | MIT |  |
| 83 | `sortedcontainers` | 2.4.0 | Apache 2.0 |  |
| 84 | `sqlalchemy` | 2.1.1 | MIT |  |
| 85 | `sympy` | 1.14.0 | BSD-3-Clause |  |
| 86 | `threadpoolctl` | 3.7.0 | BSD-3-Clause |  |
| 87 | `torch` | 2.14.0 | BSD-3-Clause; bundled: Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2/3-Clause, BSL-1.0, MIT; ships libiomp5md.dll | ตรวจที่มาและเงื่อนไขของ `libiomp5md.dll` (Intel OpenMP) — ไม่พบ notice ใน wheel |
| 88 | `torch_audiomentations` | 0.12.0 | MIT |  |
| 89 | `torch_pitch_shift` | 1.2.5 | MIT |  |
| 90 | `torchaudio` | 2.11.0 | BSD-2-Clause |  |
| 91 | `torchcodec` | 0.17.0 | BSD-3-Clause (+ bundled image codecs: BSD/zlib/libpng/libjpeg-turbo) |  |
| 92 | `torchmetrics` | 1.9.0 | Apache-2.0 |  |
| 93 | `tqdm` | 4.70.1 | MPL-2.0 AND MIT | MPL-2.0 บางส่วน: แจกได้ ไม่แก้ไฟล์ ต้องเก็บ notice |
| 94 | `truststore` | 0.10.4 | MIT |  |
| 95 | `typing_extensions` | 4.16.0 | PSF-2.0 |  |
| 96 | `tzdata` | 2026.4 | Apache-2.0 |  |
| 97 | `urllib3` | 2.8.0 | MIT |  |
| 98 | `yarl` | 1.25.1 | Apache-2.0 |  |

## Version Diff

| Version | Change |
| --- | --- |
| 0.1.0b | First licence audit of the 98 locked diarization runtime packages. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.0b | 2026-11-26 | draft | Audited PyPI metadata and wheel licence files; flagged Intel OpenMP and GCC runtime; proposed direct-from-PyPI download. | pending | Claude |
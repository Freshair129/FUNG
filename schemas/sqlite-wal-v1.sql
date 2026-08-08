-- version: 0.1.1b
-- updated_at: 2026-07-20T21:34:00+07:00
-- scope: transitional FUNG-owned SQLite schema; migration input only after GenesisBlockDB operational-boundary cutover
-- warning: this is not the Genesis signed WAL or a public Genesis relational schema package

PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  storage_path TEXT NOT NULL,
  active_recording_id TEXT,
  archived_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS recordings (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  source TEXT NOT NULL CHECK (source IN ('microphone', 'import')),
  input_path TEXT,
  canonical_audio_path TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'recording', 'paused', 'completed', 'failed')),
  duration_ms INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audio_chunks (
  id TEXT PRIMARY KEY,
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  sequence_no INTEGER NOT NULL,
  file_path TEXT NOT NULL,
  start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL,
  byte_size INTEGER NOT NULL DEFAULT 0,
  checksum TEXT,
  created_at TEXT NOT NULL,
  UNIQUE (recording_id, sequence_no)
);

CREATE TABLE IF NOT EXISTS audio_layers (
  id TEXT PRIMARY KEY,
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('original', 'cleaned', 'noise_reduced_export', 'selected_clip', 'voice', 'music', 'noise')),
  file_path TEXT NOT NULL,
  source_chunk_id TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS speakers (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  key TEXT NOT NULL,
  display_name TEXT NOT NULL,
  confidence REAL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (project_id, key)
);

CREATE TABLE IF NOT EXISTS transcript_segments (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  speaker_id TEXT REFERENCES speakers(id) ON DELETE SET NULL,
  start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL,
  text TEXT NOT NULL,
  confidence REAL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_providers (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  runtime_location TEXT NOT NULL CHECK (runtime_location IN ('local', 'lan', 'cloud')),
  kind TEXT NOT NULL CHECK (kind IN ('transcription', 'diarization', 'cleanup', 'separation', 'summary_intent')),
  enabled INTEGER NOT NULL DEFAULT 1,
  config_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS model_runs (
  id TEXT PRIMARY KEY,
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  provider_id TEXT NOT NULL REFERENCES model_providers(id),
  model_name TEXT NOT NULL,
  task_kind TEXT NOT NULL,
  runtime_location TEXT NOT NULL CHECK (runtime_location IN ('local', 'lan', 'cloud')),
  input_ref TEXT NOT NULL,
  output_ref TEXT NOT NULL,
  parameters_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS summaries (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('whole_story', 'timeline', 'decisions_actions', 'speaker')),
  content TEXT NOT NULL,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  model_run_id TEXT NOT NULL REFERENCES model_runs(id),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS intent_inferences (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  speaker_id TEXT NOT NULL REFERENCES speakers(id) ON DELETE CASCADE,
  label TEXT NOT NULL,
  confidence REAL NOT NULL,
  evidence_refs_json TEXT NOT NULL DEFAULT '[]',
  model_run_id TEXT NOT NULL REFERENCES model_runs(id),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS export_artifacts (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('wav', 'mp3', 'txt', 'srt', 'vtt', 'json')),
  file_path TEXT NOT NULL,
  source_layer_id TEXT REFERENCES audio_layers(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS jobs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  type TEXT NOT NULL CHECK (type IN ('recording.capture', 'recording.recover', 'audio.cleanup', 'audio.separate', 'transcript.transcribe', 'transcript.diarize', 'summary.generate', 'intent.infer', 'export.render')),
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'paused', 'completed', 'failed', 'retrying', 'cancelled')),
  progress INTEGER NOT NULL DEFAULT 0 CHECK (progress >= 0 AND progress <= 100),
  input_refs_json TEXT NOT NULL DEFAULT '[]',
  output_refs_json TEXT NOT NULL DEFAULT '[]',
  provider_id TEXT REFERENCES model_providers(id),
  error_code TEXT,
  error_message TEXT,
  attempt_no INTEGER NOT NULL DEFAULT 1,
  started_at TEXT,
  finished_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS job_events (
  id TEXT PRIMARY KEY,
  job_id TEXT NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'paused', 'completed', 'failed', 'retrying', 'cancelled')),
  message TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_events (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  event_type TEXT NOT NULL,
  actor TEXT NOT NULL,
  payload_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_recordings_project_id ON recordings(project_id);
CREATE INDEX IF NOT EXISTS idx_audio_chunks_recording_id ON audio_chunks(recording_id, sequence_no);
CREATE INDEX IF NOT EXISTS idx_audio_layers_recording_id ON audio_layers(recording_id, kind);
CREATE INDEX IF NOT EXISTS idx_speakers_project_id ON speakers(project_id);
CREATE INDEX IF NOT EXISTS idx_segments_project_id ON transcript_segments(project_id, start_ms);
CREATE INDEX IF NOT EXISTS idx_model_runs_recording_id ON model_runs(recording_id, task_kind);
CREATE INDEX IF NOT EXISTS idx_summaries_project_id ON summaries(project_id, kind);
CREATE INDEX IF NOT EXISTS idx_intent_project_id ON intent_inferences(project_id, speaker_id);
CREATE INDEX IF NOT EXISTS idx_exports_project_id ON export_artifacts(project_id, kind);
CREATE INDEX IF NOT EXISTS idx_jobs_project_id ON jobs(project_id, status);
CREATE UNIQUE INDEX IF NOT EXISTS uq_jobs_active_recording_capture
  ON jobs(project_id, type)
  WHERE type = 'recording.capture'
    AND status IN ('queued', 'running', 'paused', 'retrying');
CREATE INDEX IF NOT EXISTS idx_job_events_job_id ON job_events(job_id, created_at);
CREATE INDEX IF NOT EXISTS idx_audit_project_id ON audit_events(project_id, created_at);

-- Phase 1 pairing (Task 5): desktop-local record of paired mobile devices.
-- NOTE: at runtime this table lives in its own WAL-mode file
-- (`paired_devices.db`), NOT in this document's namesake `fung.db`. `fung.db`
-- is a one-time-import source read by genesis_adapter::import_legacy_sqlite,
-- which matches tables by name against GenesisBlockDB's schema -- which
-- separately defines its own, differently-shaped `paired_devices` table for
-- an unrelated mobile-side capability-delegation concept (see mobile.rs /
-- genesis_adapter.rs). Co-locating this table in fung.db would let that
-- importer sweep these rows into the wrong schema. Kept here for schema
-- documentation and as future migration input only.
CREATE TABLE IF NOT EXISTS paired_devices (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  platform TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  paired_at TEXT NOT NULL,
  revoked_at TEXT,
  pairing_session_id TEXT NOT NULL
);

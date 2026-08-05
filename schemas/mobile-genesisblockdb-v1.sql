-- version: 0.3.1b
-- updated_at: 2026-07-20T21:34:00+07:00
-- status: transitional / nonconformant to the GenesisBlockDB unified operational boundary
-- scope: historical FUNG-owned SQLite prototype; migration input only after Genesis cutover
-- warning: this file is not a Genesis relational schema package and must not be used as proof of Genesis integration

CREATE TABLE IF NOT EXISTS mobile_recording_checkpoints (
  recording_id TEXT PRIMARY KEY REFERENCES recordings(id) ON DELETE CASCADE,
  safe_offset_ms INTEGER NOT NULL DEFAULT 0,
  segment_count INTEGER NOT NULL DEFAULT 0,
  last_checksum TEXT,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  current_revision_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS note_revisions (
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  evidence_label TEXT,
  author_device_id TEXT NOT NULL,
  logical_clock TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS graph_nodes (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  -- 'meeting'..'mention' are written by the knowledge-graph builder
  -- (structural meeting/speaker layer plus the LLM extraction layer).
  entity_type TEXT NOT NULL CHECK (entity_type IN ('note', 'project', 'recording', 'person', 'meeting', 'speaker', 'topic', 'decision', 'action_item', 'mention')),
  entity_id TEXT NOT NULL,
  label TEXT NOT NULL,
  position_x REAL NOT NULL DEFAULT 50,
  position_y REAL NOT NULL DEFAULT 50,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(project_id, entity_type, entity_id)
);

CREATE TABLE IF NOT EXISTS graph_edges (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  source_node_id TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
  target_node_id TEXT NOT NULL REFERENCES graph_nodes(id) ON DELETE CASCADE,
  predicate TEXT NOT NULL,
  -- 'ai_proposed' marks LLM-extracted edges. Storable, but not writable
  -- through the client-facing relation upsert command.
  epistemic_status TEXT NOT NULL CHECK (epistemic_status IN ('confirmed', 'inferred', 'evidence', 'superseded', 'disputed', 'ai_proposed')),
  provenance_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS mutation_log (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  device_id TEXT NOT NULL,
  logical_clock TEXT NOT NULL,
  entity_type TEXT NOT NULL,
  entity_id TEXT NOT NULL,
  operation TEXT NOT NULL,
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS graph_conflicts (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  entity_id TEXT NOT NULL,
  competing_revision_ids_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('open', 'resolved')),
  created_at TEXT NOT NULL,
  resolved_at TEXT
);

CREATE TABLE IF NOT EXISTS paired_devices (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  endpoint TEXT NOT NULL UNIQUE,
  trust_state TEXT NOT NULL CHECK (trust_state IN ('paired', 'revoked', 'unreachable')),
  pairing_proof_hash TEXT NOT NULL,
  capabilities_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS capability_grants (
  id TEXT PRIMARY KEY,
  device_id TEXT NOT NULL REFERENCES paired_devices(id) ON DELETE CASCADE,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  capabilities_json TEXT NOT NULL,
  expires_at TEXT,
  revoked_at TEXT,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS delegated_jobs (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  executor_device_id TEXT REFERENCES paired_devices(id),
  operation TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('queued', 'running', 'paused', 'completed', 'failed', 'cancelled')),
  progress INTEGER NOT NULL DEFAULT 0 CHECK (progress BETWEEN 0 AND 100),
  input_manifest_hash TEXT NOT NULL,
  checkpoint_json TEXT,
  observed_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS speaker_turns (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  speaker_id TEXT NOT NULL REFERENCES speakers(id) ON DELETE RESTRICT,
  start_ms INTEGER NOT NULL CHECK (start_ms >= 0),
  end_ms INTEGER NOT NULL CHECK (end_ms > start_ms),
  confidence REAL CHECK (confidence IS NULL OR (confidence >= 0 AND confidence <= 1)),
  status TEXT NOT NULL CHECK (status IN ('proposed', 'confirmed')),
  model_run_id TEXT REFERENCES model_runs(id) ON DELETE SET NULL,
  overlap INTEGER NOT NULL DEFAULT 0 CHECK (overlap IN (0, 1)),
  revision INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS speaker_timeline_revisions (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  operation TEXT NOT NULL CHECK (operation IN ('rename', 'split', 'merge', 'confirm')),
  payload_json TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS waveform_tiles (
  id TEXT PRIMARY KEY,
  recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE CASCADE,
  zoom_level INTEGER NOT NULL,
  tile_index INTEGER NOT NULL,
  start_ms INTEGER NOT NULL,
  end_ms INTEGER NOT NULL,
  peaks_json TEXT NOT NULL,
  checksum TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(recording_id, zoom_level, tile_index)
);

CREATE TABLE IF NOT EXISTS story_sequences (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0),
  current_revision INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS story_clips (
  id TEXT PRIMARY KEY,
  sequence_id TEXT NOT NULL REFERENCES story_sequences(id) ON DELETE CASCADE,
  source_turn_id TEXT REFERENCES speaker_turns(id) ON DELETE SET NULL,
  source_recording_id TEXT NOT NULL REFERENCES recordings(id) ON DELETE RESTRICT,
  source_start_ms INTEGER NOT NULL CHECK (source_start_ms >= 0),
  source_end_ms INTEGER NOT NULL CHECK (source_end_ms > source_start_ms),
  timeline_start_ms INTEGER NOT NULL CHECK (timeline_start_ms >= 0),
  speaker_id TEXT NOT NULL REFERENCES speakers(id) ON DELETE RESTRICT,
  effect_chain_id TEXT REFERENCES effect_chains(id) ON DELETE SET NULL,
  revision INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS story_revisions (
  id TEXT PRIMARY KEY,
  sequence_id TEXT NOT NULL REFERENCES story_sequences(id) ON DELETE CASCADE,
  operation TEXT NOT NULL,
  before_json TEXT NOT NULL,
  after_json TEXT NOT NULL,
  applied INTEGER NOT NULL DEFAULT 1 CHECK (applied IN (0, 1)),
  author_device_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS voice_profiles (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  display_name TEXT NOT NULL,
  rights_basis TEXT NOT NULL CHECK (rights_basis IN ('owned_recording', 'licensed_pack', 'explicit_consent')),
  rights_evidence_ref TEXT NOT NULL,
  rights_state TEXT NOT NULL CHECK (rights_state IN ('valid', 'revoked', 'expired')),
  provider_id TEXT REFERENCES model_providers(id) ON DELETE SET NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS effect_chains (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  owner_kind TEXT NOT NULL CHECK (owner_kind IN ('project', 'story_clip', 'voice_profile')),
  owner_id TEXT NOT NULL,
  label TEXT NOT NULL,
  bypassed INTEGER NOT NULL DEFAULT 0 CHECK (bypassed IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS effect_nodes (
  id TEXT PRIMARY KEY,
  chain_id TEXT NOT NULL REFERENCES effect_chains(id) ON DELETE CASCADE,
  position INTEGER NOT NULL CHECK (position >= 0),
  kind TEXT NOT NULL CHECK (kind IN ('pitch_shift', 'reverb', 'delay', 'compressor', 'low_pass')),
  parameters_json TEXT NOT NULL,
  bypassed INTEGER NOT NULL DEFAULT 0 CHECK (bypassed IN (0, 1)),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(chain_id, position)
);

CREATE TABLE IF NOT EXISTS model_packages (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  provider_kind TEXT NOT NULL,
  model_version TEXT NOT NULL,
  size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
  checksum TEXT,
  runtime_location TEXT NOT NULL CHECK (runtime_location IN ('mobile', 'desktop', 'cloud')),
  install_state TEXT NOT NULL CHECK (install_state IN ('installed', 'available', 'incompatible', 'unknown')),
  compatibility_json TEXT NOT NULL DEFAULT '{}',
  languages_json TEXT NOT NULL DEFAULT '[]',
  license_ref TEXT,
  observed_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS transcript_refinement_proposals (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  transcript_segment_id TEXT REFERENCES transcript_segments(id) ON DELETE SET NULL,
  original_text TEXT NOT NULL,
  proposed_text TEXT NOT NULL,
  policy TEXT NOT NULL,
  model_run_id TEXT REFERENCES model_runs(id) ON DELETE SET NULL,
  status TEXT NOT NULL CHECK (status IN ('proposed', 'accepted', 'rejected', 'partially_accepted')),
  reviewed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS agent_voice_grants (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  mcp_client_id TEXT NOT NULL,
  voice_profile_id TEXT NOT NULL REFERENCES voice_profiles(id) ON DELETE CASCADE,
  capability TEXT NOT NULL CHECK (capability = 'voice.speak'),
  granted_at TEXT NOT NULL,
  expires_at TEXT,
  revoked_at TEXT,
  UNIQUE(project_id, mcp_client_id, voice_profile_id, capability)
);

CREATE TABLE IF NOT EXISTS agent_voice_sessions (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  mcp_client_id TEXT NOT NULL,
  voice_profile_id TEXT NOT NULL REFERENCES voice_profiles(id) ON DELETE RESTRICT,
  grant_id TEXT NOT NULL REFERENCES agent_voice_grants(id) ON DELETE RESTRICT,
  requested_text_hash TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('queued', 'speaking', 'muted', 'stopped', 'completed', 'failed')),
  retain_output INTEGER NOT NULL DEFAULT 0 CHECK (retain_output IN (0, 1)),
  stop_actor TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_notes_project_updated ON notes(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_graph_nodes_project ON graph_nodes(project_id, entity_type);
CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON graph_edges(project_id, source_node_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON graph_edges(project_id, target_node_id);
CREATE INDEX IF NOT EXISTS idx_mutation_project_clock ON mutation_log(project_id, logical_clock);
CREATE INDEX IF NOT EXISTS idx_speaker_turns_viewport ON speaker_turns(recording_id, start_ms, end_ms);
CREATE INDEX IF NOT EXISTS idx_speaker_turns_speaker ON speaker_turns(project_id, speaker_id);
CREATE INDEX IF NOT EXISTS idx_waveform_tiles_viewport ON waveform_tiles(recording_id, zoom_level, tile_index);
CREATE INDEX IF NOT EXISTS idx_story_sequences_project ON story_sequences(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_story_clips_sequence ON story_clips(sequence_id, timeline_start_ms);
CREATE INDEX IF NOT EXISTS idx_story_revisions_sequence ON story_revisions(sequence_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_effect_nodes_chain ON effect_nodes(chain_id, position);
CREATE INDEX IF NOT EXISTS idx_refinement_project_status ON transcript_refinement_proposals(project_id, status, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_agent_voice_sessions_active ON agent_voice_sessions(project_id, state, updated_at DESC);

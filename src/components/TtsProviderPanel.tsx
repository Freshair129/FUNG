// src/components/TtsProviderPanel.tsx

import { useEffect, useState } from "react";
import {
  Volume2, X, Plus, Play, Pencil, ToggleLeft, ToggleRight, Loader2,
} from "lucide-react";
import {
  listModelProviders,
  ttsProviderRegister,
  ttsProviderUpdate,
  ttsProviderToggle,
  ttsProviderTest,
  type ModelProvider,
} from "../tauri";
import type {
  TtsRuntimeType, TtsTestResult, TtsValidation,
} from "../mobile/model";
import "./TtsProviderPanel.css";

type Props = { onClose: () => void; embedded?: boolean };

type FormState = {
  label: string;
  runtimeType: TtsRuntimeType;
  venvPath: string;
  scriptPath: string;
  modelPath: string;
  device: "cuda" | "cpu";
  endpoint: string;
  authHeader: string;
  binaryPath: string;
  argsTemplate: string;
};

const emptyForm: FormState = {
  label: "", runtimeType: "python_script",
  venvPath: "", scriptPath: "", modelPath: "", device: "cuda",
  endpoint: "", authHeader: "",
  binaryPath: "", argsTemplate: "--text {text} --output {output}",
};

// When editing an existing provider, listModelProviders does not return the
// stored config_json, so the form only ever starts pre-filled with the
// label. If the user only changes the label and clicks update, the
// runtime-specific fields are empty — sending that as configJson would wipe
// the provider's working config. This checks whether the fields relevant to
// the selected runtime type are all empty, so the caller can omit
// configJson entirely and let the backend keep the existing config.
function isConfigFormEmpty(form: FormState): boolean {
  switch (form.runtimeType) {
    case "python_script":
      return !form.venvPath.trim() && !form.scriptPath.trim();
    case "rest_api":
      return !form.endpoint.trim();
    case "local_binary":
      return !form.binaryPath.trim();
  }
}

function buildConfigJson(form: FormState): string {
  switch (form.runtimeType) {
    case "python_script":
      return JSON.stringify({
        runtime_type: "python_script",
        venv_path: form.venvPath,
        script_path: form.scriptPath,
        ...(form.modelPath ? { model_path: form.modelPath } : {}),
        device: form.device,
      });
    case "rest_api":
      return JSON.stringify({
        runtime_type: "rest_api",
        endpoint: form.endpoint,
        ...(form.authHeader ? { auth_header: form.authHeader } : {}),
      });
    case "local_binary":
      return JSON.stringify({
        runtime_type: "local_binary",
        binary_path: form.binaryPath,
        ...(form.modelPath ? { model_path: form.modelPath } : {}),
        args_template: form.argsTemplate,
      });
  }
}

export function TtsProviderPanel({ onClose, embedded }: Props) {
  const [providers, setProviders] = useState<ModelProvider[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [editId, setEditId] = useState<string | null>(null);
  const [form, setForm] = useState<FormState>(emptyForm);
  // Test results are keyed by provider id so testing one provider never
  // clobbers or misattributes the result shown on another provider's card.
  const [testResults, setTestResults] = useState<Record<string, TtsTestResult>>({});
  const [testingId, setTestingId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [validation, setValidation] = useState<TtsValidation | null>(null);

  const loadProviders = async () => {
    const all = await listModelProviders();
    setProviders(all.filter((p) => p.kind === "tts"));
  };

  useEffect(() => { void loadProviders(); }, []);

  const handleSave = async () => {
    setSaving(true);
    setValidation(null);
    try {
      if (editId) {
        // Only send configJson when the user actually filled in runtime
        // fields — otherwise omit it so the backend preserves the existing
        // config instead of overwriting it with empty strings.
        const configJson = isConfigFormEmpty(form) ? undefined : buildConfigJson(form);
        const v = await ttsProviderUpdate(editId, form.label, configJson);
        setValidation(v);
        if (!v.ok) return;
      } else {
        const configJson = buildConfigJson(form);
        const r = await ttsProviderRegister(form.label, configJson);
        setValidation(r.validation);
        if (!r.validation.ok) return;
      }
      setShowForm(false);
      setEditId(null);
      setForm(emptyForm);
      await loadProviders();
    } catch (e) {
      setValidation({
        ok: false,
        error: e instanceof Error ? e.message : String(e),
        warnings: [],
      });
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async (providerId: string) => {
    setTestingId(providerId);
    try {
      const result = await ttsProviderTest(providerId);
      const typed: TtsTestResult = {
        status: result.status === "ok" ? "ok" : "error",
        latencyMs: result.latencyMs,
        audioPath: result.audioPath,
        message: result.message,
      };
      setTestResults((prev) => ({ ...prev, [providerId]: typed }));
      if (typed.status === "ok" && typed.audioPath) {
        const audio = new Audio(`asset://localhost/${typed.audioPath}`);
        audio.play().catch(() => {});
      }
    } catch (e) {
      setTestResults((prev) => ({
        ...prev,
        [providerId]: { status: "error", message: e instanceof Error ? e.message : String(e) },
      }));
    } finally {
      setTestingId(null);
    }
  };

  const handleToggle = async (id: string, enabled: boolean) => {
    await ttsProviderToggle(id, !enabled);
    await loadProviders();
  };

  const handleEdit = (p: ModelProvider) => {
    // Pre-fill form from provider — listModelProviders does not return the
    // stored config_json, so only the label can be pre-filled here. The user
    // re-enters the runtime-specific fields to update the config.
    setEditId(p.id);
    setForm({ ...emptyForm, label: p.label });
    setShowForm(true);
  };

  const f = (key: keyof FormState) => (e: React.ChangeEvent<HTMLInputElement | HTMLSelectElement>) =>
    setForm((prev) => ({ ...prev, [key]: e.target.value } as FormState));

  const content = (
    <div className="tts-panel" onClick={(e) => e.stopPropagation()}>
      <h2>
        <Volume2 size={18} />
        ผู้ให้บริการเสียงสังเคราะห์
        <button className="tts-panel-close" onClick={onClose}><X size={16} /></button>
      </h2>

      {/* Provider cards */}
      {providers.map((p) => {
        const result = testResults[p.id];
        const isTesting = testingId === p.id;
        const indicatorClass = !p.enabled ? "off"
          : isTesting ? "warn"
          : result ? result.status
          : "warn";
        return (
          <div key={p.id} className="tts-provider-card">
            <div className="tts-provider-card-header">
              <span className={`indicator ${indicatorClass}`} />
              <strong>{p.label}</strong>
            </div>
            <div className="tts-provider-card-meta">
              {p.runtimeLocation} · {p.enabled ? "เปิดใช้งาน" : "ปิดใช้งาน"}
            </div>
            <div className="tts-provider-card-actions">
              <button onClick={() => void handleTest(p.id)} disabled={testingId !== null}>
                {isTesting ? <Loader2 size={12} className="spin" /> : <Play size={12} />}
                {" "}ทดสอบ
              </button>
              <button onClick={() => handleEdit(p)}><Pencil size={12} /> แก้ไข</button>
              <button onClick={() => void handleToggle(p.id, p.enabled)}>
                {p.enabled ? <ToggleRight size={12} /> : <ToggleLeft size={12} />}
                {" "}{p.enabled ? "ปิดใช้งาน" : "เปิดใช้งาน"}
              </button>
            </div>
            {result && !isTesting && (
              <div className={`tts-test-result ${result.status}`}>
                {result.status === "ok"
                  ? `✅ สำเร็จ · ${result.latencyMs} ms`
                  : `❌ ${result.message}`}
              </div>
            )}
          </div>
        );
      })}

      {/* Empty state */}
      {providers.length === 0 && !showForm && (
        <div className="tts-empty">
          <Volume2 size={32} />
          <p>ยังไม่ได้ตั้งค่า TTS provider</p>
          <p>เพิ่ม provider เพื่อใช้งานเสียงสังเคราะห์</p>
        </div>
      )}

      {/* Add button */}
      {!showForm && (
        <button
          className="btn-secondary"
          style={{ width: "100%", marginTop: 12 }}
          onClick={() => { setShowForm(true); setEditId(null); setForm(emptyForm); }}
        >
          <Plus size={14} /> เพิ่ม TTS Provider
        </button>
      )}

      {/* Registration form */}
      {showForm && (
        <div className="tts-form">
          <label>ประเภท</label>
          <select value={form.runtimeType} onChange={f("runtimeType")}>
            <option value="python_script">Python Script</option>
            <option value="rest_api">REST API</option>
            <option value="local_binary">Local Binary</option>
          </select>

          <label>ชื่อ</label>
          <input value={form.label} onChange={f("label")} placeholder="เช่น F5-TTS-THAI" />

          {form.runtimeType === "python_script" && (
            <>
              <label>Venv path</label>
              <input value={form.venvPath} onChange={f("venvPath")} placeholder="D:\tts\.venv" />
              <label>Script path</label>
              <input value={form.scriptPath} onChange={f("scriptPath")} placeholder="D:\tts\synthesize.py" />
              <label>Model path (optional)</label>
              <input value={form.modelPath} onChange={f("modelPath")} placeholder="D:\tts\models\v1" />
              <label>Device</label>
              <div className="tts-form-row">
                <label><input type="radio" name="device" value="cuda" checked={form.device === "cuda"} onChange={f("device")} /> CUDA</label>
                <label><input type="radio" name="device" value="cpu" checked={form.device === "cpu"} onChange={f("device")} /> CPU</label>
              </div>
            </>
          )}

          {form.runtimeType === "rest_api" && (
            <>
              <label>Endpoint URL</label>
              <input value={form.endpoint} onChange={f("endpoint")} placeholder="http://127.0.0.1:5000/synthesize" />
              <label>Authorization header (optional)</label>
              <input value={form.authHeader} onChange={f("authHeader")} placeholder="Bearer ..." />
            </>
          )}

          {form.runtimeType === "local_binary" && (
            <>
              <label>Binary path</label>
              <input value={form.binaryPath} onChange={f("binaryPath")} placeholder="C:\piper\piper.exe" />
              <label>Model path (optional)</label>
              <input value={form.modelPath} onChange={f("modelPath")} />
              <label>Arguments template</label>
              <input value={form.argsTemplate} onChange={f("argsTemplate")} placeholder="--text {text} --output {output}" />
            </>
          )}

          {/* Validation feedback */}
          {validation && !validation.ok && (
            <div className="tts-test-result error">❌ {validation.error}</div>
          )}
          {validation && validation.warnings.length > 0 && (
            <div className="tts-warnings">
              ⚠️ {validation.warnings.join(" · ")}
            </div>
          )}

          <div className="tts-form-actions">
            <button className="btn-primary" onClick={() => void handleSave()} disabled={saving || !form.label}>
              {saving ? "กำลังบันทึก..." : editId ? "อัปเดต" : "บันทึก"}
            </button>
            <button className="btn-secondary" onClick={() => { setShowForm(false); setEditId(null); }}>
              ยกเลิก
            </button>
          </div>
        </div>
      )}
    </div>
  );

  if (embedded) return content;

  return (
    <div className="tts-panel-overlay" onClick={onClose}>
      {content}
    </div>
  );
}

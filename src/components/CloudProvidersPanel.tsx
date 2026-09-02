// src/components/CloudProvidersPanel.tsx
import { useCallback, useEffect, useState } from "react";
import { Cloud, KeyRound, X } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import "./CloudProvidersPanel.css";

type TaskKind = "stt" | "llm";

interface CloudConfigStatus {
  provider: string;
  taskKind: TaskKind;
  configured: boolean;
}

interface TierPolicy {
  sttCloudEnabled: boolean;
  llmCloudEnabled: boolean;
  dailyCap: number;
}

interface CloudCallCounts {
  stt: number;
  llm: number;
}

const PROVIDER_LABELS: Record<string, string> = { anthropic: "Anthropic", openai: "OpenAI", custom: "กำหนดเอง (Custom)" };

// Placeholder = the backend's fallback when no override is saved; must match
// DEFAULT_ANTHROPIC_MODEL / DEFAULT_OPENAI_STT_MODEL / DEFAULT_OPENAI_LLM_MODEL
// in src-tauri/src/cloud_executor.rs. "custom" has no model concept, so no entry.
const DEFAULT_MODEL_PLACEHOLDERS: Record<string, string> = {
  "anthropic-llm": "claude-3-5-sonnet-20241022",
  "openai-stt": "whisper-1",
  "openai-llm": "gpt-4o-mini",
};

interface CloudProvidersPanelProps {
  onClose: () => void;
  embedded?: boolean;
}

export function CloudProvidersPanel({ onClose, embedded }: CloudProvidersPanelProps) {
  const [statuses, setStatuses] = useState<CloudConfigStatus[]>([]);
  const [policy, setPolicy] = useState<TierPolicy>({ sttCloudEnabled: false, llmCloudEnabled: false, dailyCap: 20 });
  const [counts, setCounts] = useState<CloudCallCounts>({ stt: 0, llm: 0 });
  const [keyDrafts, setKeyDrafts] = useState<Record<string, string>>({});
  const [endpointDrafts, setEndpointDrafts] = useState<Record<string, string>>({});
  const [modelDrafts, setModelDrafts] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  // Local typing buffer for the daily-cap input, kept separate from `policy`
  // so every keystroke doesn't call savePolicy: typing "20" -> "100" would
  // otherwise transiently save dailyCap: 1 after the first keystroke, which
  // could momentarily block a cloud dispatch mid-edit. Committed to `policy`
  // (and the backend) on blur.
  const [dailyCapDraft, setDailyCapDraft] = useState<string>("20");

  const refresh = useCallback(async () => {
    try {
      const [nextStatuses, nextPolicy, nextCounts] = await Promise.all([
        invoke<CloudConfigStatus[]>("cloud_config_status"),
        invoke<TierPolicy>("tier_policy_get"),
        invoke<CloudCallCounts>("cloud_call_counts_today"),
      ]);
      setStatuses(nextStatuses);
      setPolicy(nextPolicy);
      setDailyCapDraft(String(nextPolicy.dailyCap));
      setCounts(nextCounts);
    } catch (e) {
      console.error("Failed to load cloud provider state:", e);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const draftKey = (provider: string, taskKind: TaskKind) => `${provider}-${taskKind}`;

  const saveKey = useCallback(
    async (provider: string, taskKind: TaskKind) => {
      setError(null);
      const key = draftKey(provider, taskKind);
      try {
        const validation = await invoke<{ ok: boolean; error: string | null }>("cloud_config_set", {
          input: {
            provider,
            taskKind,
            apiKey: keyDrafts[key] ?? "",
            endpoint: provider === "custom" ? endpointDrafts[key] ?? "" : null,
            // Optional override; null/absent keeps the backend's default model
            // (serde(default) on CloudConfigInput.model stays backward compatible).
            model: provider === "custom" ? null : (modelDrafts[key] ?? "").trim() || null,
          },
        });
        if (!validation.ok) {
          setError(validation.error ?? "บันทึกไม่สำเร็จ");
          return;
        }
        setKeyDrafts((prev) => ({ ...prev, [key]: "" }));
        void refresh();
      } catch (e) {
        setError(e instanceof Error ? e.message : "บันทึกไม่สำเร็จ");
      }
    },
    [keyDrafts, endpointDrafts, modelDrafts, refresh],
  );

  const clearKey = useCallback(
    async (provider: string, taskKind: TaskKind) => {
      try {
        await invoke("cloud_config_clear", { provider, taskKind });
        void refresh();
      } catch (e) {
        console.error("Failed to clear cloud config:", e);
      }
    },
    [refresh],
  );

  const savePolicy = useCallback(
    async (next: TierPolicy) => {
      setPolicy(next);
      try {
        await invoke("tier_policy_set", { policy: next });
      } catch (e) {
        console.error("Failed to save tier policy:", e);
        void refresh();
      }
    },
    [refresh],
  );

  // (anthropic, llm) / (openai, stt) / (openai, llm) / (custom, stt) / (custom, llm) —
  // Anthropic has no STT product, matching cloud_config_status's fixed slot list.
  const slots: Array<{ provider: string; taskKind: TaskKind }> = [
    { provider: "anthropic", taskKind: "llm" },
    { provider: "openai", taskKind: "stt" },
    { provider: "openai", taskKind: "llm" },
    { provider: "custom", taskKind: "stt" },
    { provider: "custom", taskKind: "llm" },
  ];

  const content = (
    <section
      className="cloud-providers-panel"
      aria-label="ผู้ให้บริการคลาวด์"
      aria-modal="true"
      role="dialog"
      onMouseDown={(e) => e.stopPropagation()}
    >
      <header className="cloud-providers-header">
        <Cloud size={18} />
        <h2>ผู้ให้บริการคลาวด์</h2>
        <button type="button" className="cloud-providers-close" onClick={onClose} aria-label="ปิด">
          <X size={16} />
        </button>
      </header>

      <section className="cloud-providers-section">
        <h3>คีย์ API</h3>
        {slots.map(({ provider, taskKind }) => {
          const key = draftKey(provider, taskKind);
          const status = statuses.find((s) => s.provider === provider && s.taskKind === taskKind);
          return (
            <div key={key} className="cloud-providers-card">
              <div className="cloud-providers-card-title">
                <strong>{PROVIDER_LABELS[provider]}</strong>
                <small>{taskKind === "stt" ? "ถอดเสียง (STT)" : "LLM"}</small>
                {status?.configured && <span className="cloud-providers-badge">ตั้งค่าแล้ว ✓</span>}
              </div>
              {provider === "custom" && (
                <>
                  <input
                    className="cloud-providers-input"
                    type="text"
                    placeholder="https://your-endpoint.example.com"
                    value={endpointDrafts[key] ?? ""}
                    onChange={(e) => setEndpointDrafts((prev) => ({ ...prev, [key]: e.target.value }))}
                  />
                  <p className="cloud-providers-hint">
                    ใส่คีย์ในช่อง API key ด้านล่าง อย่าใส่ไว้ใน URL
                  </p>
                </>
              )}
              <input
                className="cloud-providers-input"
                type="password"
                placeholder="API key"
                value={keyDrafts[key] ?? ""}
                onChange={(e) => setKeyDrafts((prev) => ({ ...prev, [key]: e.target.value }))}
              />
              {provider !== "custom" && (
                <label className="cloud-providers-field">
                  <span>โมเดล (เว้นว่างเพื่อใช้ค่าเริ่มต้น)</span>
                  <input
                    className="cloud-providers-input"
                    type="text"
                    placeholder={DEFAULT_MODEL_PLACEHOLDERS[key]}
                    value={modelDrafts[key] ?? ""}
                    onChange={(e) => setModelDrafts((prev) => ({ ...prev, [key]: e.target.value }))}
                  />
                </label>
              )}
              <div className="cloud-providers-card-actions">
                <button type="button" onClick={() => void saveKey(provider, taskKind)}>
                  <KeyRound size={14} /> บันทึก
                </button>
                {status?.configured && (
                  <button type="button" className="cloud-providers-clear" onClick={() => void clearKey(provider, taskKind)}>
                    ลบ
                  </button>
                )}
              </div>
            </div>
          );
        })}
        <p className="cloud-providers-hint">
          สำหรับ LLM หากตั้งค่าหลายผู้ให้บริการ ระบบจะใช้ตามลำดับ: Anthropic → OpenAI → กำหนดเอง
        </p>
      </section>

      <section className="cloud-providers-section">
        <h3>นโยบายลำดับการประมวลผล</h3>
        <div className="cloud-providers-card">
          <p className="cloud-providers-chain">อุปกรณ์นี้ → เดสก์ท็อปที่จับคู่ → คลาวด์</p>
          <label className="cloud-providers-toggle-row">
            <span>อนุญาตให้ใช้คลาวด์สำหรับถอดเสียง (STT)</span>
            <button
              type="button"
              className={`cloud-providers-switch ${policy.sttCloudEnabled ? "is-on" : ""}`}
              role="switch"
              aria-checked={policy.sttCloudEnabled}
              onClick={() => void savePolicy({ ...policy, sttCloudEnabled: !policy.sttCloudEnabled })}
            >
              <i />
            </button>
          </label>
          <label className="cloud-providers-toggle-row">
            <span>อนุญาตให้ใช้คลาวด์สำหรับ LLM</span>
            <button
              type="button"
              className={`cloud-providers-switch ${policy.llmCloudEnabled ? "is-on" : ""}`}
              role="switch"
              aria-checked={policy.llmCloudEnabled}
              onClick={() => void savePolicy({ ...policy, llmCloudEnabled: !policy.llmCloudEnabled })}
            >
              <i />
            </button>
          </label>
        </div>
      </section>

      <section className="cloud-providers-section">
        <h3>ขีดจำกัดต่อวัน</h3>
        <div className="cloud-providers-card">
          <label className="cloud-providers-field">
            <span>จำนวนครั้งสูงสุดต่อวัน (ต่อประเภทงาน)</span>
            <input
              className="cloud-providers-input cloud-providers-input--narrow"
              type="number"
              min={1}
              value={dailyCapDraft}
              onChange={(e) => setDailyCapDraft(e.target.value)}
              onBlur={() => {
                const dailyCap = Math.max(1, Number(dailyCapDraft) || 1);
                setDailyCapDraft(String(dailyCap));
                void savePolicy({ ...policy, dailyCap });
              }}
            />
          </label>
          <dl className="cloud-providers-counts">
            <div><dt>ถอดเสียงวันนี้</dt><dd>{counts.stt} / {policy.dailyCap}</dd></div>
            <div><dt>LLM วันนี้</dt><dd>{counts.llm} / {policy.dailyCap}</dd></div>
          </dl>
        </div>
      </section>

      {error && <p className="cloud-providers-error">{error}</p>}
    </section>
  );

  if (embedded) return content;

  return (
    <div className="cloud-providers-overlay" role="presentation" onMouseDown={onClose}>
      {content}
    </div>
  );
}

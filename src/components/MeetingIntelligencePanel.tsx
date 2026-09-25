import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  applyTranscriptEvent,
  buildHumanCorrectionRequest,
  canUseLocalDrafting,
  createRequestId,
  isAgentRevisionCurrent,
  MEETING_EVENT_BUFFER_LIMIT,
  MEETING_INTELLIGENCE_EXTERNAL_DISPATCH_AVAILABLE,
  MEETING_REPLAY_PAGE_SIZE,
  mergeReplayEvents,
  type AgentMode,
  type AgentCapability,
  type KnowledgeCollection,
  type KnowledgeCollectionClassification,
  type KnowledgeImportReceipt,
  type KnowledgeMetricBasis,
  type KnowledgeMetricComputeResult,
  type KnowledgeMetricSaveReceipt,
  type LocalDeliveryPreview,
  type MeetingAgentStatus,
  type MeetingHistoryEntry,
  type MeetingIntelligenceService,
  type MeetingScope,
  type OwnerVaultOption,
  type PrivateDraft,
  type TranscriptEvent,
  type TranscriptSnapshot,
  type TranscriptUtterance,
} from "../lib/meetingIntelligence.ts";
import { MeetingPeoplePanel } from "./MeetingPeoplePanel.tsx";
import "./MeetingIntelligencePanel.css";

export type MeetingIntelligencePanelProps = MeetingScope & {
  service: MeetingIntelligenceService;
  onClose?: () => void;
};

const HISTORY_LIMIT = 30;
const REPLAY_PAGE_LIMIT = 20;

function failureMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (/command .* not found|unknown command|not registered/i.test(message)) {
    return "Native meeting-intelligence commands are not registered in this build yet.";
  }
  if (/CURSOR_EXPIRED/i.test(message)) {
    return "Transcript history expired. Reload the current snapshot before continuing.";
  }
  if (/MEETING_TRANSCRIPT_CURSOR_STALE/.test(message)) {
    return "Transcript changed while preparing this draft. Refresh it and create a new draft before preview or approval.";
  }
  if (/MEETING_AGENT_MODEL_(NOT_INSTALLED|NOT_CONFIGURED|ENDPOINT_INVALID|UNAVAILABLE|TIMEOUT|CONFIG_STALE)/.test(message)) {
    return "โมเดลท้องถิ่นไม่พร้อมหรือการตั้งค่าเปลี่ยนระหว่างสร้างร่าง ตรวจชื่อโมเดลและ Ollama แล้วลองใหม่";
  }
  if (/MEETING_AGENT_MODEL_(OUTPUT_INVALID|REFS_INVALID|INPUT_INVALID|NAME_INVALID)/.test(message)) {
    return "ข้อเสนอจากโมเดลไม่ผ่านการตรวจรูปแบบหรือหลักฐาน จึงไม่สร้างร่างใหม่";
  }
  if (/MEETING_KNOWLEDGE_METRIC_CONFLICT/.test(message)) {
    return "พบตัวเลขจากหลายแหล่งที่ขัดแย้งกัน จึงไม่คำนวณผลให้";
  }
  if (/MEETING_KNOWLEDGE_METRIC_NOT_COMPUTABLE/.test(message)) {
    return "ยังไม่มี Actual และ Budget ที่ตรงกับช่วงเวลาและหน่วยนี้ครบคู่";
  }
  if (/MEETING_KNOWLEDGE_METRIC_AMBIGUOUS/.test(message)) {
    return "มีตัวเลขมากกว่าหนึ่งรายการในขอบเขตที่เลือก กรุณาตรวจหลักฐานก่อนคำนวณ";
  }
  if (/PDF_PARSER_SANDBOX_UNAVAILABLE|PDF_PARSER_SANDBOX_UNSUPPORTED/.test(message)) {
    return "PDF ต้องใช้ parser runtime ที่แยกไว้และ Windows AppContainer; ระบบปฏิเสธการอ่านเมื่อ sandbox ไม่พร้อม";
  }
  return "The local meeting-intelligence action could not be completed.";
}

function formatMetricDecimal(value: { coefficient: string; scale: number }): string {
  const negative = value.coefficient.startsWith("-");
  const digits = (negative ? value.coefficient.slice(1) : value.coefficient).padStart(value.scale + 1, "0");
  const formatted = value.scale === 0
    ? digits
    : `${digits.slice(0, -value.scale)}.${digits.slice(-value.scale)}`.replace(/0+$/, "").replace(/\.$/, "");
  return negative && formatted !== "0" ? `-${formatted}` : formatted;
}

function stateLabel(state: MeetingAgentStatus["state"]): string {
  switch (state) {
    case "stopped": return "หยุดอยู่";
    case "starting": return "กำลังเริ่ม";
    case "observing": return "สังเกตในเครื่อง";
    case "drafting": return "ร่างคำตอบในเครื่อง";
    case "paused": return "พักชั่วคราว";
    case "blocked": return "ถูกบล็อกตามนโยบาย";
  }
}

function modeLabel(mode: AgentMode): string {
  if (mode === "off") return "ปิด";
  if (mode === "observe") return "สังเกตในเครื่อง";
  return "ร่างคำตอบส่วนตัว";
}

function readinessLabel(readiness: string): string {
  if (readiness === "ready") return "พร้อม";
  if (readiness === "blocked") return "ถูกบล็อก";
  return "ยังไม่พร้อม";
}

function utteranceStateLabel(state: TranscriptUtterance["state"]): string {
  if (state === "provisional") return "ชั่วคราว";
  if (state === "committed") return "ยืนยันแล้ว";
  if (state === "reviewed") return "ตรวจแก้แล้ว";
  return "ถอนออก";
}

function historyLabel(entry: MeetingHistoryEntry): string {
  switch (entry.kind) {
    case "mode_changed": return `เปลี่ยนโหมดเป็น ${entry.state}`;
    case "draft_created": return "สร้างร่างคำตอบส่วนตัว";
    case "draft_blocked": return "บล็อกร่างตามนโยบาย";
    case "preview_created": return "สร้างตัวอย่างในเครื่อง";
    case "approval_recorded": return "อนุมัติตัวอย่างในเครื่อง";
    case "revoked": return "เพิกถอนสิทธิ์ในเครื่อง";
  }
}

function sortEvents(events: TranscriptEvent[]): TranscriptEvent[] {
  return [...events].sort((left, right) => left.cursor - right.cursor);
}

export function MeetingIntelligencePanel({
  projectId,
  recordingId,
  service,
  onClose,
}: MeetingIntelligencePanelProps) {
  const scope = useMemo(() => ({ projectId, recordingId }), [projectId, recordingId]);
  const generationRef = useRef(0);
  const accountLifecycleRef = useRef(0);
  const agentRevisionRef = useRef(-1);
  const snapshotRef = useRef<TranscriptSnapshot | null>(null);
  const draftRef = useRef<PrivateDraft | null>(null);
  const vaultUnlockedRef = useRef(false);
  const replayActiveRef = useRef(false);
  const pendingLiveEventsRef = useRef<TranscriptEvent[]>([]);
  const [snapshot, setSnapshot] = useState<TranscriptSnapshot | null>(null);
  const selectedPeopleScope = snapshot?.utterances.find((row) => row.speakerId)?.scope
    ?? snapshot?.utterances[0]?.scope
    ?? null;
  const peopleProjectId = selectedPeopleScope?.project_id ?? null;
  const peopleRecordingId = selectedPeopleScope?.recording_id ?? null;
  const peopleMeetingSessionId = selectedPeopleScope?.meeting_session_id ?? null;
  const peopleSourceSessionId = selectedPeopleScope?.source_session_id ?? null;
  const peopleTrackId = selectedPeopleScope?.track_id ?? null;
  const peopleSourceGeneration = selectedPeopleScope?.source_generation ?? null;
  const peopleScope = useMemo(() => {
    if (
      !peopleProjectId
      || !peopleRecordingId
      || !peopleMeetingSessionId
      || !peopleSourceSessionId
      || !peopleTrackId
      || peopleSourceGeneration === null
    ) return null;
    return {
      project_id: peopleProjectId,
      recording_id: peopleRecordingId,
      meeting_session_id: peopleMeetingSessionId,
      source_session_id: peopleSourceSessionId,
      track_id: peopleTrackId,
      source_generation: peopleSourceGeneration,
    };
  }, [
    peopleMeetingSessionId,
    peopleProjectId,
    peopleRecordingId,
    peopleSourceGeneration,
    peopleSourceSessionId,
    peopleTrackId,
  ]);
  const [collections, setCollections] = useState<KnowledgeCollection[]>([]);
  const [vaultOptions, setVaultOptions] = useState<OwnerVaultOption[]>([]);
  const [selectedVaultId, setSelectedVaultId] = useState<string | null>(null);
  const [vaultUnlockedInPanel, setVaultUnlockedInPanel] = useState(false);
  const [collectionClassification, setCollectionClassification] = useState<KnowledgeCollectionClassification>("internal");
  const [ownerVaultNotice, setOwnerVaultNotice] = useState<string | null>(null);
  const [lastImportReceipt, setLastImportReceipt] = useState<KnowledgeImportReceipt | null>(null);
  const [metricKey, setMetricKey] = useState("");
  const [metricOrganizationRef, setMetricOrganizationRef] = useState("");
  const [metricPeriodStart, setMetricPeriodStart] = useState("");
  const [metricPeriodEnd, setMetricPeriodEnd] = useState("");
  const [metricCalendar, setMetricCalendar] = useState("gregorian");
  const [metricUnit, setMetricUnit] = useState("");
  const [metricCurrency, setMetricCurrency] = useState("");
  const [metricScale, setMetricScale] = useState("raw");
  const [metricBasis, setMetricBasis] = useState<KnowledgeMetricBasis>("actual");
  const [metricValue, setMetricValue] = useState("");
  const [metricChunkId, setMetricChunkId] = useState("");
  const [metricSaveReceipt, setMetricSaveReceipt] = useState<KnowledgeMetricSaveReceipt | null>(null);
  const [metricResult, setMetricResult] = useState<KnowledgeMetricComputeResult | null>(null);
  const [agentStatus, setAgentStatus] = useState<MeetingAgentStatus | null>(null);
  const [history, setHistory] = useState<MeetingHistoryEntry[]>([]);
  const [collectionRevision, setCollectionRevision] = useState(0);
  const [accountLifecycleRevision, setAccountLifecycleRevision] = useState(0);
  const [draft, setDraft] = useState<PrivateDraft | null>(null);
  const [deliveryPreview, setDeliveryPreview] = useState<LocalDeliveryPreview | null>(null);
  const [mode, setMode] = useState<AgentMode>("off");
  const [question, setQuestion] = useState("");
  const [draftKind, setDraftKind] = useState<"extractive" | "model_proposal">("extractive");
  const [modelName, setModelName] = useState("");
  const [modelReadiness, setModelReadiness] = useState<AgentCapability | null>(null);
  const [correctionText, setCorrectionText] = useState<Record<string, string>>({});
  const [selectedUtterance, setSelectedUtterance] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [transcriptLoading, setTranscriptLoading] = useState(false);
  const [timelineIncomplete, setTimelineIncomplete] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [policyNotice, setPolicyNotice] = useState<string | null>(null);

  const publishSnapshot = useCallback((next: TranscriptSnapshot | null) => {
    snapshotRef.current = next;
    setSnapshot(next);
  }, []);

  const publishDraft = useCallback((next: PrivateDraft | null) => {
    draftRef.current = next;
    setDraft(next);
  }, []);

  const publishAgentStatus = useCallback((next: MeetingAgentStatus) => {
    if (!isAgentRevisionCurrent(agentRevisionRef.current, next.revision)) return;
    agentRevisionRef.current = next.revision;
    setAgentStatus(next);
    setMode(next.mode);
    if (next.localAgent.readiness !== "ready" || next.transcriptRead.readiness !== "ready" || next.knowledgeRead.readiness !== "ready") {
      publishDraft(null);
      setDeliveryPreview(null);
    }
  }, [publishDraft]);

  const refreshVaultOptions = useCallback(async (expectedGeneration: number) => {
    const accountLifecycle = accountLifecycleRef.current;
    try {
      const options = await service.vaultOptions();
      if (generationRef.current !== expectedGeneration || accountLifecycleRef.current !== accountLifecycle) return;
      setVaultOptions(options);
      setSelectedVaultId((current) => current && options.some((option) => option.vaultId === current) ? current : null);
      setOwnerVaultNotice(null);
    } catch {
      if (generationRef.current !== expectedGeneration || accountLifecycleRef.current !== accountLifecycle) return;
      setVaultOptions([]);
      setSelectedVaultId(null);
      vaultUnlockedRef.current = false;
      setVaultUnlockedInPanel(false);
      setOwnerVaultNotice("ยังอ่านรายการ local owner vault ไม่ได้ใน build นี้");
    }
  }, [service]);

  const readTranscript = useCallback(async (expectedGeneration: number): Promise<boolean> => {
    setTranscriptLoading(true);
    replayActiveRef.current = true;
    pendingLiveEventsRef.current = [];
    try {
      let current = await service.snapshot(recordingId, projectId);
      if (generationRef.current !== expectedGeneration) return false;
      let cursor = current.cursor;
      let incomplete = false;

      for (let pageNumber = 0; pageNumber < REPLAY_PAGE_LIMIT; pageNumber += 1) {
        const page = await service.replay({ projectId, recordingId, afterCursor: cursor }, MEETING_REPLAY_PAGE_SIZE);
        if (generationRef.current !== expectedGeneration) return false;
        const merged = mergeReplayEvents(current, page.events);
        current = merged.snapshot;
        if (merged.needsReplay) {
          incomplete = true;
          break;
        }
        if (!page.hasMore) {
          if (current.cursor < page.highWatermark) incomplete = true;
          break;
        }
        if (page.nextCursor <= cursor) {
          incomplete = true;
          break;
        }
        cursor = page.nextCursor;
        if (pageNumber === REPLAY_PAGE_LIMIT - 1) incomplete = true;
      }

      const pending = mergeReplayEvents(current, sortEvents(pendingLiveEventsRef.current));
      current = pending.snapshot;
      if (pending.needsReplay || pendingLiveEventsRef.current.length >= MEETING_EVENT_BUFFER_LIMIT) incomplete = true;

      publishSnapshot(current);
      setTimelineIncomplete(incomplete);
      if (draftRef.current && current.cursor > draftRef.current.basedOnTranscriptCursor) {
        publishDraft({ ...draftRef.current, state: "stale" });
        setDeliveryPreview(null);
      }
      return !incomplete;
    } catch (readError) {
      if (generationRef.current === expectedGeneration) setError(failureMessage(readError));
      return false;
    } finally {
      replayActiveRef.current = false;
      pendingLiveEventsRef.current = [];
      if (generationRef.current === expectedGeneration) setTranscriptLoading(false);
    }
  }, [projectId, recordingId, service, publishDraft, publishSnapshot]);

  const refreshAgentData = useCallback(async (expectedGeneration: number) => {
    const accountLifecycle = accountLifecycleRef.current;
    const results = await Promise.allSettled([
      service.preflight(scope),
      service.status(scope),
      service.collections(projectId, recordingId),
      service.history(scope, HISTORY_LIMIT),
    ]);
    if (generationRef.current !== expectedGeneration || accountLifecycleRef.current !== accountLifecycle) return;
    const [preflightResult, statusResult, collectionResult, historyResult] = results;
    if (preflightResult.status === "fulfilled") {
      const nextStatus = preflightResult.value.status;
      publishAgentStatus(nextStatus);
      setCollectionRevision(preflightResult.value.collectionRevision);
    } else if (statusResult.status === "fulfilled") {
      publishAgentStatus(statusResult.value);
    }
    if (collectionResult.status === "fulfilled") setCollections(collectionResult.value);
    if (historyResult.status === "fulfilled") setHistory(historyResult.value);
    if (preflightResult.status === "rejected" && statusResult.status === "rejected") {
      setError(failureMessage(preflightResult.reason));
    }
  }, [projectId, recordingId, scope, service, publishAgentStatus]);

  useEffect(() => {
    const generation = generationRef.current + 1;
    generationRef.current = generation;
    let disposed = false;
    let transcriptReady = false;
    let bufferOverflowed = false;
    let bufferedTranscriptEvents: TranscriptEvent[] = [];
    let unlisten: (() => void) | null = null;
    publishSnapshot(null);
    publishDraft(null);
    setCollections([]);
    setVaultOptions([]);
    setSelectedVaultId(null);
    vaultUnlockedRef.current = false;
    setVaultUnlockedInPanel(false);
    setCollectionClassification("internal");
    setOwnerVaultNotice(null);
    setLastImportReceipt(null);
    setMetricSaveReceipt(null);
    setMetricResult(null);
    setMetricChunkId("");
    setMetricKey("");
    setMetricOrganizationRef("");
    setMetricPeriodStart("");
    setMetricPeriodEnd("");
    setMetricCalendar("gregorian");
    setMetricUnit("");
    setMetricCurrency("");
    setMetricScale("raw");
    setMetricBasis("actual");
    setAgentStatus(null);
    agentRevisionRef.current = -1;
    setHistory([]);
    setDeliveryPreview(null);
    setMode("off");
    setTimelineIncomplete(false);
    setError(null);
    setPolicyNotice(null);
    setLoading(true);

    const acceptTranscript = (event: TranscriptEvent) => {
      if (disposed || event.recordingId !== recordingId || event.projectId !== projectId) return;
      if (!transcriptReady) {
        if (bufferedTranscriptEvents.length >= MEETING_EVENT_BUFFER_LIMIT) {
          bufferedTranscriptEvents.shift();
          bufferOverflowed = true;
        }
        bufferedTranscriptEvents.push(event);
        return;
      }
      if (replayActiveRef.current) {
        if (pendingLiveEventsRef.current.length >= MEETING_EVENT_BUFFER_LIMIT) {
          pendingLiveEventsRef.current.shift();
          setTimelineIncomplete(true);
        }
        pendingLiveEventsRef.current.push(event);
        return;
      }
      const current = snapshotRef.current;
      if (!current) return;
      const applied = applyTranscriptEvent(current, event);
      if (applied.needsReplay) {
        setTimelineIncomplete(true);
        return;
      }
      if (!applied.accepted) return;
      publishSnapshot(applied.snapshot);
      if (draftRef.current && event.cursor > draftRef.current.basedOnTranscriptCursor) {
        publishDraft({ ...draftRef.current, state: "stale" });
        setDeliveryPreview(null);
      }
    };

    const subscribe = async () => {
      try {
        const dispose = await service.subscribe(scope, {
          onTranscript: acceptTranscript,
          onAgentStatus: (event) => {
            if (!disposed && event.recordingId === recordingId && event.projectId === projectId && isAgentRevisionCurrent(agentRevisionRef.current, event.revision)) {
              agentRevisionRef.current = event.revision;
              setAgentStatus((current) => current && current.revision > event.revision
                ? current
                : { ...event, expiresAt: current?.expiresAt ?? null, localAgent: current?.localAgent ?? { readiness: "unavailable", reasonCode: "NOT_REFRESHED" }, transcriptRead: current?.transcriptRead ?? { readiness: "unavailable", reasonCode: "NOT_REFRESHED" }, knowledgeRead: current?.knowledgeRead ?? { readiness: "unavailable", reasonCode: "NOT_REFRESHED" }, automaticTrigger: event.automaticTrigger ?? current?.automaticTrigger ?? { readiness: event.mode === "draft" ? "blocked" : "unavailable", reasonCode: event.mode === "draft" ? "TRUSTED_PARTICIPANT_ATTRIBUTION_UNAVAILABLE" : "AGENT_MODE_NOT_DRAFT" }, externalJoin: { readiness: "unavailable", reasonCode: "PROVIDER_UNCONFIGURED" }, externalMediaRead: { readiness: "unavailable", reasonCode: "PROVIDER_UNCONFIGURED" }, externalChatSend: { readiness: "unavailable", reasonCode: "PROVIDER_UNCONFIGURED" }, externalLinkSend: { readiness: "unavailable", reasonCode: "PROVIDER_UNCONFIGURED" }, externalFileUpload: { readiness: "unavailable", reasonCode: "PROVIDER_UNCONFIGURED" } });
              setMode(event.mode);
            }
          },
          onDraft: (event) => {
            if (!disposed && vaultUnlockedRef.current && event.recordingId === recordingId && event.projectId === projectId) {
              const currentCursor = snapshotRef.current?.cursor ?? event.draft.basedOnTranscriptCursor;
              const next = currentCursor > event.draft.basedOnTranscriptCursor
                ? { ...event.draft, state: "stale" as const }
                : event.draft;
              publishDraft(next);
              setDeliveryPreview(null);
            }
          },
          onPolicyBlocked: (event) => {
            if (!disposed && event.recordingId === recordingId && event.projectId === projectId) {
              setPolicyNotice(`${event.code}: ${event.reason}`);
              setDeliveryPreview(null);
              if (draftRef.current) publishDraft({ ...draftRef.current, state: "blocked" });
            }
          },
          onDelivery: (event) => {
            if (!disposed && event.recordingId === recordingId && event.projectId === projectId) {
              setPolicyNotice(event.state === "approved_local_only"
                ? "อนุมัติเฉพาะตัวอย่างในเครื่องแล้ว ยังไม่มีการส่งออกไปยังห้องประชุม"
                : `สถานะตัวอย่างในเครื่อง: ${event.state}`);
            }
          },
          onAccountLifecycleChanged: () => {
            accountLifecycleRef.current += 1;
            setAccountLifecycleRevision(accountLifecycleRef.current);
            vaultUnlockedRef.current = false;
            setVaultUnlockedInPanel(false);
            setVaultOptions([]);
            setSelectedVaultId(null);
            setCollections([]);
            setCollectionRevision(0);
            setLastImportReceipt(null);
            setMetricSaveReceipt(null);
            setMetricResult(null);
            setMetricChunkId("");
            setMetricKey("");
            setMetricOrganizationRef("");
            setMetricPeriodStart("");
            setMetricPeriodEnd("");
            setMetricCalendar("gregorian");
            setMetricUnit("");
            setMetricCurrency("");
            setMetricScale("raw");
            setMetricBasis("actual");
            setMetricValue("");
            setAgentStatus(null);
            setHistory([]);
            setMode("off");
            setBusy(null);
            setDeliveryPreview(null);
            setPolicyNotice(null);
            setOwnerVaultNotice("บัญชีเปลี่ยนหรือออกจากระบบแล้ว · ล็อก local owner vault ในแผงนี้");
            publishDraft(null);
            void service.lockVault().catch(() => undefined);
          },
        });
        if (disposed) dispose();
        else unlisten = dispose;
      } catch (subscribeError) {
        if (!disposed) setError(failureMessage(subscribeError));
      }
    };

    const bootstrap = async () => {
      try {
        await subscribe();
        const timelinePromise = readTranscript(generation);
        await refreshAgentData(generation);
        await refreshVaultOptions(generation);
        await timelinePromise;
        if (disposed || generationRef.current !== generation) return;
        let current = snapshotRef.current;
        if (current) {
          const merged = mergeReplayEvents(current, sortEvents(bufferedTranscriptEvents));
          current = merged.snapshot;
          if (merged.needsReplay || bufferOverflowed) setTimelineIncomplete(true);
          publishSnapshot(current);
        }
        transcriptReady = true;
      } catch (bootstrapError) {
        if (!disposed) setError(failureMessage(bootstrapError));
      } finally {
        if (!disposed && generationRef.current === generation) setLoading(false);
      }
    };

    void bootstrap();
    return () => {
      disposed = true;
      transcriptReady = false;
      generationRef.current += 1;
      unlisten?.();
      bufferedTranscriptEvents = [];
    };
  }, [projectId, recordingId, scope, service, publishDraft, publishSnapshot, readTranscript, refreshAgentData, refreshVaultOptions]);

  const runOperation = useCallback(async <T,>(
    label: string,
    operation: () => Promise<T>,
    onSuccess: (value: T) => void,
  ) => {
    const generation = generationRef.current;
    const accountLifecycle = accountLifecycleRef.current;
    setBusy(label);
    setError(null);
    try {
      const result = await operation();
      if (generationRef.current === generation && accountLifecycleRef.current === accountLifecycle) onSuccess(result);
    } catch (operationError) {
      if (generationRef.current === generation && accountLifecycleRef.current === accountLifecycle) setError(failureMessage(operationError));
    } finally {
      if (generationRef.current === generation && accountLifecycleRef.current === accountLifecycle) setBusy(null);
    }
  }, []);

  const mutation = useCallback((expectedRevision = agentStatus?.revision ?? 0) => ({
    ...scope,
    requestId: createRequestId("mi"),
    expectedRevision,
  }), [scope, agentStatus?.revision]);

  const selectedVault = vaultOptions.find((option) => option.vaultId === selectedVaultId) ?? null;
  const canDraftLocally = canUseLocalDrafting(vaultUnlockedInPanel, timelineIncomplete, agentStatus);
  useEffect(() => {
    if (draftKind !== "model_proposal" || !canDraftLocally || !modelName.trim()) {
      setModelReadiness(null);
      return;
    }
    let cancelled = false;
    const timer = window.setTimeout(() => {
      void service.modelReadiness(modelName.trim()).then(
        (result) => { if (!cancelled) setModelReadiness(result); },
        () => { if (!cancelled) setModelReadiness({ readiness: "blocked", reasonCode: "MEETING_AGENT_MODEL_UNAVAILABLE" }); },
      );
    }, 350);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [canDraftLocally, draftKind, modelName, service]);

  const provisionOwnerVault = useCallback(() => {
    if (busy || vaultUnlockedInPanel) return;
    void runOperation("vault-provision", () => service.provisionVault(), (vaultId) => {
      setSelectedVaultId(vaultId);
      vaultUnlockedRef.current = false;
      setVaultUnlockedInPanel(false);
      setOwnerVaultNotice("สร้าง vault แล้ว · กดปลดล็อกแยกต่างหากเพื่อใช้งาน");
      void refreshVaultOptions(generationRef.current);
    });
  }, [busy, refreshVaultOptions, runOperation, service, vaultUnlockedInPanel]);

  const unlockOwnerVault = useCallback(() => {
    if (busy || vaultUnlockedInPanel || !selectedVault || !selectedVault.keyAvailable) return;
    void runOperation("vault-unlock", () => service.unlockVault(selectedVault.vaultId), () => {
      vaultUnlockedRef.current = true;
      setVaultUnlockedInPanel(true);
      setOwnerVaultNotice("ปลดล็อก vault ผ่านแผงนี้แล้ว");
      setLastImportReceipt(null);
      void refreshAgentData(generationRef.current);
    });
  }, [busy, refreshAgentData, runOperation, selectedVault, service, vaultUnlockedInPanel]);

  const lockOwnerVault = useCallback(() => {
    if (busy || !vaultUnlockedInPanel) return;
    vaultUnlockedRef.current = false;
    setVaultUnlockedInPanel(false);
    publishDraft(null);
    setDeliveryPreview(null);
    const selectedCollectionIds = collections.filter((collection) => collection.selected).map((collection) => collection.collectionId);
    void runOperation("vault-lock", async () => {
      if (selectedCollectionIds.length) {
        try {
          await service.setCollections({
            ...scope,
            collectionIds: [],
            expectedRevision: collectionRevision,
            requestId: createRequestId("knowledge-selection-lock"),
          });
        } catch {
          // Lock remains the priority; the native vault lock rechecks all access.
        }
      }
      try {
        await service.lockVault();
      } catch (lockError) {
        vaultUnlockedRef.current = false;
        setVaultUnlockedInPanel(false);
        setCollections((current) => current.map((collection) => ({ ...collection, selected: false })));
        publishDraft(null);
        setDeliveryPreview(null);
        setMode("off");
        setLastImportReceipt(null);
        setMetricSaveReceipt(null);
        setMetricResult(null);
        setMetricChunkId("");
        setMetricKey("");
        setMetricOrganizationRef("");
        setMetricPeriodStart("");
        setMetricPeriodEnd("");
        setMetricCalendar("gregorian");
        setMetricUnit("");
        setMetricCurrency("");
        setMetricScale("raw");
        setMetricBasis("actual");
        setMetricValue("");
        setOwnerVaultNotice("ไม่ยืนยันผลล็อก vault · ปิดการใช้ knowledge ในแผงนี้จนกว่าจะปลดล็อกใหม่");
        throw lockError;
      }
    }, () => {
      vaultUnlockedRef.current = false;
      setVaultUnlockedInPanel(false);
      setCollections((current) => current.map((collection) => ({ ...collection, selected: false })));
      publishDraft(null);
      setDeliveryPreview(null);
      setMode("off");
      setLastImportReceipt(null);
      setMetricSaveReceipt(null);
      setMetricResult(null);
      setMetricChunkId("");
      setMetricKey("");
      setMetricOrganizationRef("");
      setMetricPeriodStart("");
      setMetricPeriodEnd("");
      setMetricCalendar("gregorian");
      setMetricUnit("");
      setMetricCurrency("");
      setMetricScale("raw");
      setMetricBasis("actual");
      setMetricValue("");
      setOwnerVaultNotice("ส่งคำสั่งล็อก vault สำเร็จ · สถานะเปิดจริงต้องยืนยันจาก native service");
      void refreshAgentData(generationRef.current);
    });
  }, [busy, collectionRevision, collections, publishDraft, refreshAgentData, runOperation, scope, service, vaultUnlockedInPanel]);

  const createKnowledgeCollection = useCallback(() => {
    if (busy || !vaultUnlockedInPanel) return;
    void runOperation("collection-create", () => service.createCollection(projectId, collectionClassification), ({ collectionId }) => {
      setOwnerVaultNotice(`สร้าง collection แล้ว: ${collectionId}`);
      void refreshAgentData(generationRef.current);
    });
  }, [busy, collectionClassification, projectId, refreshAgentData, runOperation, service, vaultUnlockedInPanel]);

  const importDocument = useCallback((collection: KnowledgeCollection) => {
    if (busy || !vaultUnlockedInPanel || !collection.readable) return;
    void runOperation("document-import", () => service.importSelectedDocument(projectId, collection.collectionId), (receipt) => {
      setLastImportReceipt(receipt);
      setMetricChunkId(receipt.citationLocators[0]?.chunkId ?? "");
      setMetricSaveReceipt(null);
      setMetricResult(null);
      setOwnerVaultNotice(`นำเข้าเอกสารในเครื่องแล้ว · ${receipt.chunkCount} chunks`);
      void refreshAgentData(generationRef.current);
    });
  }, [busy, projectId, refreshAgentData, runOperation, service, vaultUnlockedInPanel]);

  const saveMetricObservation = useCallback(() => {
    const selectedCollectionIds = collections.filter((row) => row.selected && row.readable).map((row) => row.collectionId);
    if (busy || !vaultUnlockedInPanel || !lastImportReceipt || !selectedCollectionIds.includes(lastImportReceipt.collectionId) || !metricChunkId) return;
    void runOperation("metric-save", () => service.saveMetric({
      ...scope,
      requestId: createRequestId("knowledge-metric"),
      collectionIds: selectedCollectionIds,
      documentVersionId: lastImportReceipt.versionId,
      chunkId: metricChunkId,
      metricKey: metricKey.trim(),
      organizationRef: metricOrganizationRef.trim(),
      periodStart: metricPeriodStart,
      periodEnd: metricPeriodEnd,
      calendar: metricCalendar.trim(),
      unit: metricUnit.trim(),
      currency: metricCurrency.trim() || null,
      scale: metricScale.trim(),
      basis: metricBasis,
      value: metricValue.trim(),
    }), (receipt) => {
      setMetricSaveReceipt(receipt);
      setMetricResult(null);
    });
  }, [busy, collections, lastImportReceipt, metricBasis, metricCalendar, metricChunkId, metricCurrency, metricKey, metricOrganizationRef, metricPeriodEnd, metricPeriodStart, metricScale, metricUnit, metricValue, runOperation, scope, service, vaultUnlockedInPanel]);

  const computeMetricComparison = useCallback(() => {
    const selectedCollectionIds = collections.filter((row) => row.selected && row.readable).map((row) => row.collectionId);
    if (busy || !vaultUnlockedInPanel || !selectedCollectionIds.length) return;
    void runOperation("metric-compute", () => service.computeMetric({
      ...scope,
      collectionIds: selectedCollectionIds,
      metricKey: metricKey.trim(),
      organizationRef: metricOrganizationRef.trim(),
      periodStart: metricPeriodStart,
      periodEnd: metricPeriodEnd,
      calendar: metricCalendar.trim(),
      unit: metricUnit.trim(),
      currency: metricCurrency.trim() || null,
      scale: metricScale.trim(),
    }), setMetricResult);
  }, [busy, collections, metricCalendar, metricCurrency, metricKey, metricOrganizationRef, metricPeriodEnd, metricPeriodStart, metricScale, metricUnit, runOperation, scope, service, vaultUnlockedInPanel]);

  const applyMode = useCallback(() => {
    if (mode === "draft" && !canDraftLocally) return;
    if (mode === "off") {
      void runOperation("stop", () => service.stop(mutation()), (result) => {
        publishAgentStatus(result);
        setMode("off");
        setDeliveryPreview(null);
        publishDraft(null);
        void refreshAgentData(generationRef.current);
      });
      return;
    }
    if (agentStatus?.state === "stopped" || !agentStatus) {
      void runOperation("start", () => service.start({ ...mutation(), mode }), (result) => {
        publishAgentStatus(result);
        setMode(result.mode);
        void refreshAgentData(generationRef.current);
      });
      return;
    }
    void runOperation("policy", () => service.setPolicy({
      ...mutation(),
      mode,
      allowedTopics: agentStatus.allowedTopics ?? [],
      expiresAt: agentStatus.expiresAt,
    }), (result) => {
      publishAgentStatus(result);
      setMode(result.mode);
      publishDraft(null);
      setDeliveryPreview(null);
      void refreshAgentData(generationRef.current);
    });
  }, [agentStatus, canDraftLocally, mode, mutation, publishAgentStatus, publishDraft, refreshAgentData, runOperation, service]);

  const toggleCollection = useCallback((collection: KnowledgeCollection) => {
    if (!collection.readable || !vaultUnlockedInPanel || busy) return;
    const nextIds = collections
      .filter((row) => row.selected !== (row.collectionId === collection.collectionId))
      .filter((row) => row.readable)
      .map((row) => row.collectionId);
    const currentlySelected = collections.some((row) => row.collectionId === collection.collectionId && row.selected);
    const collectionIds = currentlySelected
      ? nextIds
      : [...new Set([...nextIds, collection.collectionId])];
    void runOperation("collections", () => service.setCollections({
      ...scope,
      collectionIds,
      expectedRevision: collectionRevision,
      requestId: createRequestId("knowledge-selection"),
    }), (result) => {
      setCollections((current) => current.map((row) => ({
        ...row,
        selected: result.selectedCollectionIds.includes(row.collectionId),
      })));
      setCollectionRevision(result.revision);
      publishDraft(null);
      setDeliveryPreview(null);
      void refreshAgentData(generationRef.current);
    });
  }, [busy, collectionRevision, collections, publishDraft, runOperation, scope, service, refreshAgentData, vaultUnlockedInPanel]);

  const submitQuestion = useCallback(() => {
    const text = question.trim();
    const selectedCollectionIds = collections.filter((row) => row.selected && row.readable).map((row) => row.collectionId);
    if (!text || !snapshot || mode !== "draft" || !canDraftLocally || selectedCollectionIds.length === 0 || busy) return;
    void runOperation("ask", () => service.ask({
      ...mutation(),
      question: text,
      collectionIds: selectedCollectionIds,
      transcriptCursor: snapshot.cursor,
      draftKind,
      modelName: draftKind === "model_proposal" ? modelName.trim() : null,
    }), (result) => {
      if (!vaultUnlockedRef.current) return;
      publishDraft(result);
      setDeliveryPreview(null);
      setQuestion("");
      void refreshAgentData(generationRef.current);
    });
  }, [agentStatus?.revision, busy, canDraftLocally, collections, draftKind, mode, modelName, mutation, question, refreshAgentData, runOperation, service, snapshot, publishDraft]);

  const prepareLocalPreview = useCallback(() => {
    if (!canDraftLocally || !draft || draft.state !== "private" || busy) return;
    void runOperation("preview", () => service.previewDelivery({
      ...mutation(),
      draftId: draft.draftId,
      draftRevision: draft.revision,
      scope: "local_preview_only",
    }), (result) => {
      setDeliveryPreview(result);
      void refreshAgentData(generationRef.current);
    });
  }, [busy, canDraftLocally, draft, mutation, refreshAgentData, runOperation, service]);

  const approveLocalPreview = useCallback(() => {
    if (!deliveryPreview || deliveryPreview.state !== "awaiting_approval" || busy) return;
    void runOperation("approve-preview", () => service.approveDelivery({
      ...mutation(),
      intentId: deliveryPreview.intentId,
      approvedPayloadHash: deliveryPreview.payloadHash,
      scope: "local_preview_only",
    }), (result) => {
      setDeliveryPreview(result);
      setPolicyNotice("อนุมัติเฉพาะตัวอย่างในเครื่องแล้ว ไม่มีการส่งข้อความหรือลิงก์");
      void refreshAgentData(generationRef.current);
    });
  }, [busy, deliveryPreview, mutation, refreshAgentData, runOperation, service]);

  const revoke = useCallback(() => {
    if (busy) return;
    void runOperation("revoke", () => service.revoke({
      ...mutation(),
      reason: "operator_requested",
    }), (result) => {
      publishAgentStatus(result);
      setMode("off");
      publishDraft(null);
      setDeliveryPreview(null);
      setPolicyNotice("เพิกถอนสิทธิ์และยกเลิกงานที่ยังไม่เริ่มแล้ว");
      void refreshAgentData(generationRef.current);
    });
  }, [busy, mutation, publishAgentStatus, publishDraft, refreshAgentData, runOperation, service]);

  const correctUtterance = useCallback((utterance: TranscriptUtterance) => {
    const correctedText = (correctionText[utterance.utteranceId] ?? utterance.text).trim();
    if (!correctedText || correctedText === utterance.text || busy) return;
    void runOperation("correction", () => service.correct(buildHumanCorrectionRequest(utterance, correctedText)), (receipt) => {
      setSelectedUtterance(null);
      setCorrectionText((current) => {
        const next = { ...current };
        delete next[utterance.utteranceId];
        return next;
      });
      if (snapshotRef.current && receipt.cursor > snapshotRef.current.cursor) setTimelineIncomplete(true);
    });
  }, [busy, correctionText, runOperation, scope, service]);

  const externalReadiness = agentStatus ? [
    ["เข้าร่วม Meet", agentStatus.externalJoin],
    ["รับสื่อจากห้อง", agentStatus.externalMediaRead],
    ["ส่งข้อความ", agentStatus.externalChatSend],
    ["ส่งลิงก์", agentStatus.externalLinkSend],
    ["อัปโหลดไฟล์", agentStatus.externalFileUpload],
  ] as const : [];

  return (
    <section className="meeting-intelligence" aria-labelledby="meeting-intelligence-title">
      <header className="meeting-intelligence__header">
        <div>
          <p className="meeting-intelligence__eyebrow">FUNG · LOCAL MEETING INTELLIGENCE</p>
          <h2 id="meeting-intelligence-title">ผู้ช่วยประชุมในเครื่อง</h2>
          <p className="meeting-intelligence__sub">ตรวจ transcript ตาม revision และจัดการ vault/knowledge สำหรับ recording นี้ · คำถามส่วนตัวจะแสดงเมื่อ native readiness ครบ</p>
        </div>
        {onClose && <button type="button" className="meeting-intelligence__button" onClick={onClose}>ปิด</button>}
      </header>

      <div className="meeting-intelligence__notice" role="status">
        ทำงานในเครื่องเท่านั้น · การเข้าร่วม Meet การรับสื่อ และการส่งข้อความ/ลิงก์/ไฟล์ยังไม่พร้อมใช้งาน
      </div>
      {error && <p className="meeting-intelligence__error" role="alert">{error}</p>}
      {policyNotice && <p className="meeting-intelligence__policy" role="status">{policyNotice}</p>}

      <div className="meeting-intelligence__grid">
        <section className="meeting-intelligence__card meeting-intelligence__transcript" aria-labelledby="mi-transcript-title">
          <div className="meeting-intelligence__card-heading">
            <div>
              <h3 id="mi-transcript-title">Transcript ตาม revision</h3>
              <p>{snapshot ? `cursor ${snapshot.cursor} · revision ${snapshot.revision}` : "รอข้อมูล transcript"}</p>
            </div>
            <button
              type="button"
              className="meeting-intelligence__button"
              disabled={transcriptLoading || loading}
              onClick={() => void readTranscript(generationRef.current)}
            >
              {transcriptLoading ? "กำลังโหลด…" : "โหลดใหม่"}
            </button>
          </div>
          {timelineIncomplete && (
            <p className="meeting-intelligence__warning" role="alert">
              event บางช่วงยังไม่ครบ จึงไม่รับรองความต่อเนื่องของรายการ · โหลด snapshot/replay ใหม่ก่อนทำงานต่อ
            </p>
          )}
          {snapshot?.legacySnapshot && (
            <p className="meeting-intelligence__warning">ข้อมูลนี้มาจาก snapshot เดิมที่ยังไม่มี replay cursor</p>
          )}
          {!snapshot?.utterances.length && !loading && <p className="meeting-intelligence__empty">ยังไม่มี transcript revision สำหรับ recording นี้</p>}
          <ol className="meeting-intelligence__utterances">
            {snapshot?.utterances.map((utterance) => {
              const editing = selectedUtterance === utterance.utteranceId;
              const editable = utterance.state === "committed" || utterance.state === "reviewed";
              return (
                <li key={utterance.utteranceId} className={`meeting-intelligence__utterance meeting-intelligence__utterance--${utterance.state}`}>
                  <div className="meeting-intelligence__utterance-meta">
                    <span>{utterance.speakerLabel ?? (utterance.actorKind === "unknown" ? "ผู้พูดไม่ทราบชื่อ" : utterance.actorKind)}</span>
                    <span>{formatTime(utterance.startMs)}–{formatTime(utterance.endMs)}</span>
                    <span>rev {utterance.revision} · {utteranceStateLabel(utterance.state)}</span>
                  </div>
                  {editing ? (
                    <>
                      <label className="meeting-intelligence__sr-only" htmlFor={`mi-correction-${utterance.utteranceId}`}>แก้ข้อความ transcript</label>
                      <textarea
                        id={`mi-correction-${utterance.utteranceId}`}
                        value={correctionText[utterance.utteranceId] ?? utterance.text}
                        maxLength={4000}
                        onChange={(event) => setCorrectionText((current) => ({ ...current, [utterance.utteranceId]: event.target.value }))}
                      />
                      <div className="meeting-intelligence__actions">
                        <button type="button" className="meeting-intelligence__button meeting-intelligence__button--primary" disabled={busy !== null} onClick={() => correctUtterance(utterance)}>บันทึก revision ใหม่</button>
                        <button type="button" className="meeting-intelligence__button" disabled={busy !== null} onClick={() => setSelectedUtterance(null)}>ยกเลิก</button>
                      </div>
                    </>
                  ) : (
                    <>
                      <p className="meeting-intelligence__utterance-text">{utterance.text}</p>
                      {editable && <button type="button" className="meeting-intelligence__text-button" disabled={busy !== null || timelineIncomplete} onClick={() => setSelectedUtterance(utterance.utteranceId)}>แก้ไขข้อความนี้</button>}
                    </>
                  )}
                </li>
              );
            })}
          </ol>
          {!!snapshot?.gaps.length && (
            <details className="meeting-intelligence__gaps">
              <summary>ช่วงเสียงที่ขาดหาย ({snapshot.gaps.length})</summary>
              <ul>{snapshot.gaps.map((gap) => <li key={gap.id}>{formatTime(gap.startMs)}–{formatTime(gap.endMs)} · {gap.reason}</li>)}</ul>
            </details>
          )}
        </section>

        <aside className="meeting-intelligence__side">
          <section className="meeting-intelligence__card" aria-labelledby="mi-knowledge-title">
            <h3 id="mi-knowledge-title">คลังความรู้ที่เลือก</h3>
            <p className="meeting-intelligence__muted">จัดการ local owner vault และ collection ในขอบเขต recording นี้ สิทธิ์ถูกตรวจซ้ำใน native service</p>
            <div className="meeting-intelligence__owner-controls">
              <p className="meeting-intelligence__vault-status" role="status">
                Vault status: {vaultUnlockedInPanel ? "ปลดล็อกผ่านแผงนี้" : "ยังไม่ได้ยืนยันการปลดล็อกผ่านแผงนี้"}
              </p>
              <label className="meeting-intelligence__field">
                เลือก local owner vault
                <select
                  aria-label="เลือก local owner vault"
                  value={selectedVaultId ?? ""}
                  disabled={busy !== null || vaultUnlockedInPanel}
                  onChange={(event) => {
                    setSelectedVaultId(event.target.value || null);
                    vaultUnlockedRef.current = false;
                    setVaultUnlockedInPanel(false);
                    publishDraft(null);
                    setDeliveryPreview(null);
                    setLastImportReceipt(null);
                    setMetricSaveReceipt(null);
                    setMetricResult(null);
                    setMetricChunkId("");
                    setMetricKey("");
                    setMetricOrganizationRef("");
                    setMetricPeriodStart("");
                    setMetricPeriodEnd("");
                    setMetricCalendar("gregorian");
                    setMetricUnit("");
                    setMetricCurrency("");
                    setMetricScale("raw");
                    setMetricBasis("actual");
                    setMetricValue("");
                  }}
                >
                  <option value="">เลือก vault…</option>
                  {vaultOptions.map((option) => (
                    <option key={option.vaultId} value={option.vaultId}>
                      {option.vaultId} · key {option.keyAvailable ? "พร้อม" : "ไม่มี"} · account {option.accountBound ? "ผูกแล้ว" : "ไม่ผูก"}
                    </option>
                  ))}
                </select>
              </label>
              <div className="meeting-intelligence__actions">
                <button type="button" className="meeting-intelligence__button" disabled={busy !== null || vaultUnlockedInPanel} onClick={provisionOwnerVault}>สร้าง owner vault</button>
                <button type="button" className="meeting-intelligence__button meeting-intelligence__button--primary" disabled={busy !== null || vaultUnlockedInPanel || !selectedVault?.keyAvailable || !selectedVault.accountBound} onClick={unlockOwnerVault}>ปลดล็อก vault</button>
                <button type="button" className="meeting-intelligence__button" disabled={busy !== null || !vaultUnlockedInPanel} onClick={lockOwnerVault}>ล็อก vault</button>
              </div>
              {ownerVaultNotice && <p className="meeting-intelligence__muted" role="status">{ownerVaultNotice}</p>}
              <p className="meeting-intelligence__muted">รายการ vault แสดง key/account binding เท่านั้น สถานะปลดล็อกในระบบยืนยันได้จาก native command เมื่อสั่งผ่านแผงนี้</p>
            </div>

            <div className="meeting-intelligence__collection-create">
              <h4>สร้าง knowledge collection</h4>
              <label className="meeting-intelligence__field">
                ระดับข้อมูล
                <select value={collectionClassification} disabled={busy !== null || !vaultUnlockedInPanel} onChange={(event) => setCollectionClassification(event.target.value as KnowledgeCollectionClassification)}>
                  <option value="internal">internal</option>
                  <option value="confidential">confidential</option>
                </select>
              </label>
              <button type="button" className="meeting-intelligence__button" disabled={busy !== null || !vaultUnlockedInPanel} onClick={createKnowledgeCollection}>สร้าง collection</button>
            </div>
            <p className="meeting-intelligence__muted">TXT/Markdown ใช้ parser ในเครื่อง · PDF ใช้เฉพาะ Windows AppContainer ที่จำกัดทรัพยากร · OCR, DOCX และ XLSX ยังไม่รองรับ</p>

            {!collections.length && <p className="meeting-intelligence__empty">ยังไม่มีคลังความรู้ที่พร้อมเลือก</p>}
            <div className="meeting-intelligence__collections">
              {collections.map((collection) => (
                <div key={collection.collectionId} className="meeting-intelligence__collection">
                  <label className="meeting-intelligence__collection-choice">
                    <input
                      type="checkbox"
                      checked={collection.selected}
                      disabled={!vaultUnlockedInPanel || !collection.readable || busy !== null}
                      onChange={() => toggleCollection(collection)}
                    />
                    <span>
                      <strong>{collection.label}</strong>
                      <small>{collection.readable ? `อ่านได้ · ${collection.classification}` : "ไม่มีสิทธิ์อ่าน"}</small>
                    </span>
                  </label>
                  <button type="button" className="meeting-intelligence__button" disabled={!vaultUnlockedInPanel || !collection.readable || busy !== null} onClick={() => importDocument(collection)}>นำเข้าเอกสารที่เลือก…</button>
                </div>
              ))}
            </div>
            {lastImportReceipt && (
              <div className="meeting-intelligence__import-receipt" role="status">
                <strong>ผลนำเข้า</strong>
                <small>document {lastImportReceipt.documentId} · version {lastImportReceipt.versionId} · {lastImportReceipt.chunkCount} chunks · parser {lastImportReceipt.parserVersion}</small>
                <code>{lastImportReceipt.contentSha256}</code>
                {!!lastImportReceipt.warnings.length && <ul>{lastImportReceipt.warnings.map((warning, index) => <li key={`${index}:${warning}`}>{warning}</li>)}</ul>}
              </div>
            )}
            {lastImportReceipt && vaultUnlockedInPanel && (
              <div className="meeting-intelligence__metric" aria-labelledby="mi-metric-title">
                <h4 id="mi-metric-title">บันทึกตัวเลขพร้อม citation</h4>
                {!collections.some((row) => row.collectionId === lastImportReceipt.collectionId && row.selected && row.readable)
                  && <p className="meeting-intelligence__muted">เลือก collection ของเอกสารนี้ก่อน เพื่อยืนยันสิทธิ์อ่านหลักฐาน</p>}
                <label className="meeting-intelligence__field">
                  แหล่ง citation
                  <select value={metricChunkId} disabled={busy !== null} onChange={(event) => setMetricChunkId(event.target.value)}>
                    <option value="">เลือกช่วงเอกสาร…</option>
                    {lastImportReceipt.citationLocators.map((locator) => <option key={locator.chunkId} value={locator.chunkId}>{locator.label}</option>)}
                  </select>
                </label>
                <div className="meeting-intelligence__metric-grid">
                  <label className="meeting-intelligence__field">Metric key<input value={metricKey} disabled={busy !== null} onChange={(event) => setMetricKey(event.target.value)} maxLength={256} /></label>
                  <label className="meeting-intelligence__field">Organization ref<input value={metricOrganizationRef} disabled={busy !== null} onChange={(event) => setMetricOrganizationRef(event.target.value)} maxLength={256} /></label>
                  <label className="meeting-intelligence__field">งวดเริ่ม<input type="date" value={metricPeriodStart} disabled={busy !== null} onChange={(event) => setMetricPeriodStart(event.target.value)} /></label>
                  <label className="meeting-intelligence__field">งวดสิ้นสุด<input type="date" value={metricPeriodEnd} disabled={busy !== null} onChange={(event) => setMetricPeriodEnd(event.target.value)} /></label>
                  <label className="meeting-intelligence__field">Calendar<input value={metricCalendar} disabled={busy !== null} onChange={(event) => setMetricCalendar(event.target.value)} maxLength={64} /></label>
                  <label className="meeting-intelligence__field">หน่วย<input value={metricUnit} disabled={busy !== null} onChange={(event) => setMetricUnit(event.target.value)} maxLength={64} /></label>
                  <label className="meeting-intelligence__field">สกุลเงิน (ถ้ามี)<input value={metricCurrency} disabled={busy !== null} onChange={(event) => setMetricCurrency(event.target.value)} maxLength={16} /></label>
                  <label className="meeting-intelligence__field">Scale<input value={metricScale} disabled={busy !== null} onChange={(event) => setMetricScale(event.target.value)} maxLength={64} /></label>
                  <label className="meeting-intelligence__field">ประเภท<select value={metricBasis} disabled={busy !== null} onChange={(event) => setMetricBasis(event.target.value as KnowledgeMetricBasis)}><option value="actual">Actual</option><option value="budget">Budget</option></select></label>
                  <label className="meeting-intelligence__field">ค่า decimal<input inputMode="decimal" value={metricValue} disabled={busy !== null} onChange={(event) => setMetricValue(event.target.value)} maxLength={40} /></label>
                </div>
                <div className="meeting-intelligence__actions">
                  <button type="button" className="meeting-intelligence__button" disabled={busy !== null || !metricChunkId || !metricKey.trim() || !metricOrganizationRef.trim() || !metricPeriodStart || !metricPeriodEnd || !metricUnit.trim() || !metricScale.trim() || !metricValue.trim() || !collections.some((row) => row.collectionId === lastImportReceipt.collectionId && row.selected && row.readable)} onClick={saveMetricObservation}>บันทึก Actual/Budget</button>
                  <button type="button" className="meeting-intelligence__button meeting-intelligence__button--primary" disabled={busy !== null || !metricKey.trim() || !metricOrganizationRef.trim() || !metricPeriodStart || !metricPeriodEnd || !metricUnit.trim() || !metricScale.trim() || !collections.some((row) => row.selected && row.readable)} onClick={computeMetricComparison}>คำนวณ Actual เทียบ Budget</button>
                </div>
                {metricSaveReceipt && <p className="meeting-intelligence__muted" role="status">บันทึก {metricSaveReceipt.basis} = {formatMetricDecimal(metricSaveReceipt.value)} · source {metricSaveReceipt.citation.documentVersionId} · {JSON.stringify(metricSaveReceipt.citation.locator)}</p>}
                {metricResult && <div className="meeting-intelligence__metric-result" role="status"><strong>ผลต่าง {formatMetricDecimal(metricResult.percentage)}%</strong>{metricResult.citations.map((citation) => <small key={`${citation.documentVersionId}:${citation.readGrantId}`}>หลักฐาน {citation.documentVersionId} · {citation.sourceVersion} · {JSON.stringify(citation.locator)}</small>)}</div>}
                <p className="meeting-intelligence__muted">ผลคำนวณต้องมี Actual/Budget อย่างละหนึ่งรายการที่ช่วงเวลา ปฏิทิน หน่วย สกุลเงิน และ scale ตรงกัน; ถ้าหลายแหล่งขัดแย้ง ระบบจะไม่เลือกเอง</p>
              </div>
            )}
          </section>

          <MeetingPeoplePanel
            projectId={projectId}
            scope={peopleScope}
            service={service}
            enabled={vaultUnlockedInPanel && !timelineIncomplete}
            accountLifecycleRevision={accountLifecycleRevision}
            accountLifecycleRef={accountLifecycleRef}
          />

          <section className="meeting-intelligence__card" aria-labelledby="mi-readiness-title">
            <div className="meeting-intelligence__card-heading">
              <div>
                <h3 id="mi-readiness-title">โหมดและความพร้อม</h3>
                <p>{agentStatus ? `${stateLabel(agentStatus.state)} · transcript cursor ${agentStatus.lastObservedTranscriptCursor}` : loading ? "กำลังตรวจสอบ…" : "ยังไม่ได้เชื่อม native service"}</p>
              </div>
            </div>
            <label className="meeting-intelligence__field">
              โหมดทำงานในเครื่อง
              <select value={mode} disabled={busy !== null} onChange={(event) => setMode(event.target.value as AgentMode)}>
                <option value="off">ปิด</option>
                <option value="observe">สังเกต transcript</option>
                <option value="draft" disabled={!canDraftLocally}>สร้างร่างส่วนตัว{canDraftLocally ? "" : " · ยังไม่พร้อม"}</option>
              </select>
            </label>
            <div className="meeting-intelligence__actions">
              <button type="button" className="meeting-intelligence__button meeting-intelligence__button--primary" disabled={busy !== null || loading || timelineIncomplete || (mode === "draft" && !canDraftLocally)} onClick={applyMode}>
                {mode === "off" ? "หยุดโหมดในเครื่อง" : agentStatus?.state === "stopped" ? "เริ่มโหมดในเครื่อง" : "ใช้โหมดนี้"}
              </button>
              {agentStatus && agentStatus.state !== "stopped" && (
                <button type="button" className="meeting-intelligence__button" disabled={busy !== null} onClick={() => void runOperation("pause", () => service.pause(mutation()), publishAgentStatus)}>พัก</button>
              )}
              <button type="button" className="meeting-intelligence__button meeting-intelligence__button--danger" disabled={busy !== null || !agentStatus} onClick={revoke}>เพิกถอนสิทธิ์</button>
            </div>
            <div className="meeting-intelligence__readiness">
              <ReadinessRow label="ผู้ช่วยในเครื่อง" readiness={agentStatus?.localAgent.readiness ?? "unavailable"} />
              <ReadinessRow label="อ่าน transcript" readiness={agentStatus?.transcriptRead.readiness ?? "unavailable"} />
              <ReadinessRow label="ค้น knowledge" readiness={agentStatus?.knowledgeRead.readiness ?? "unavailable"} />
              <ReadinessRow label="ร่างอัตโนมัติจากผู้พูด" readiness={agentStatus?.automaticTrigger.readiness ?? "unavailable"} reason={agentStatus?.automaticTrigger.reasonCode ?? "AGENT_STATUS_UNAVAILABLE"} />
              {externalReadiness.map(([label, capability]) => (
                <ReadinessRow key={label} label={label} readiness="unavailable" reason={capability.reasonCode ?? "PROVIDER_UNCONFIGURED"} />
              ))}
            </div>
            {!!agentStatus?.blockers.length && <ul className="meeting-intelligence__blockers">{agentStatus.blockers.map((blocker) => <li key={blocker}>{blocker}</li>)}</ul>}
            <p className="meeting-intelligence__muted">การอนุมัติตัวอย่างไม่อนุญาตให้ส่งออก และการเปิดแอปใหม่จะไม่เริ่ม session เอง</p>
          </section>

          {canDraftLocally ? (
            <section className="meeting-intelligence__card" aria-labelledby="mi-ask-title">
              <h3 id="mi-ask-title">ถามจากหลักฐาน</h3>
              <label className="meeting-intelligence__field">
                คำถามสำหรับร่างส่วนตัว
                <textarea
                  value={question}
                  maxLength={4000}
                  onChange={(event) => setQuestion(event.target.value)}
                  placeholder="พิมพ์คำถามเพื่อค้นเฉพาะคลังที่เลือก"
                />
              </label>
              <label className="meeting-intelligence__field">
                วิธีสร้างร่าง
                <select value={draftKind} onChange={(event) => setDraftKind(event.target.value as "extractive" | "model_proposal") }>
                  <option value="extractive">ยกข้อความหลักฐาน (ค่าเริ่มต้น)</option>
                  <option value="model_proposal">ให้โมเดลท้องถิ่นเสนอคำตอบ</option>
                </select>
              </label>
              {draftKind === "model_proposal" && (
                <>
                  <label className="meeting-intelligence__field">
                    ชื่อโมเดล Ollama ที่ติดตั้งในเครื่อง
                    <input type="text" value={modelName} maxLength={128} onChange={(event) => { setModelName(event.target.value); setModelReadiness(null); }} placeholder="เช่น llama3.1:8b" />
                  </label>
                  <p className="meeting-intelligence__muted">เลือกชื่อโมเดลให้ตรงกับที่ติดตั้งไว้; คำตอบเป็นข้อเสนอที่ต้องตรวจหลักฐานเอง</p>
                  <ReadinessRow label="โมเดลท้องถิ่น" readiness={modelReadiness?.readiness ?? "unavailable"} reason={modelReadiness?.reasonCode ?? (modelName.trim() ? "MEETING_AGENT_MODEL_CHECKING" : "MEETING_AGENT_MODEL_NAME_REQUIRED")} />
                </>
              )}
              <button
                type="button"
                className="meeting-intelligence__button meeting-intelligence__button--primary"
                disabled={busy !== null || mode !== "draft" || !snapshot || timelineIncomplete || !collections.some((row) => row.selected && row.readable) || !question.trim() || (draftKind === "model_proposal" && modelReadiness?.readiness !== "ready")}
                onClick={submitQuestion}
              >
                สร้างร่างส่วนตัว
              </button>
              {mode !== "draft" && <p className="meeting-intelligence__muted">เลือกโหมด “สร้างร่างส่วนตัว” ก่อนถาม</p>}
            </section>
          ) : (
            <section className="meeting-intelligence__card" aria-labelledby="mi-ask-title">
              <h3 id="mi-ask-title">ถามจากหลักฐาน</h3>
              <p className="meeting-intelligence__muted">ยังไม่เปิดคำถาม/ร่างส่วนตัว · ต้องปลดล็อก vault และ native status ต้องยืนยันว่า agent, transcript และ knowledge พร้อม</p>
            </section>
          )}

          {canDraftLocally && draft && (
            <section className="meeting-intelligence__card meeting-intelligence__draft" aria-labelledby="mi-draft-title">
              <div className="meeting-intelligence__card-heading">
                <div>
                  <h3 id="mi-draft-title">ร่างส่วนตัว</h3>
                  <p>draft rev {draft.revision} · transcript cursor {draft.basedOnTranscriptCursor}</p>
                  {draft.draftKind === "model_proposal" && <p>model proposal — ยังไม่ยืนยันความถูกต้อง · run {draft.modelRunId}</p>}
                </div>
                <span className={`meeting-intelligence__badge meeting-intelligence__badge--${draft.state}`}>{draft.state === "private" ? "ส่วนตัว" : draft.state === "stale" ? "ข้อมูลเปลี่ยน" : "ถูกบล็อก"}</span>
              </div>
              <p className="meeting-intelligence__draft-text">{draft.text}</p>
              <h4>หลักฐานและตำแหน่งอ้างอิง</h4>
              <ul className="meeting-intelligence__citations">
                {draft.citations.map((citation, index) => (
                  <li key={`${citation.documentId}:${citation.versionId}:${citation.locator}:${index}`}>
                    <strong>{citation.label}</strong>
                    <small>เวอร์ชัน {citation.versionId} · {citation.locator}</small>
                  </li>
                ))}
              </ul>
              {!draft.citations.length && <p className="meeting-intelligence__warning">ไม่มี citation ที่ตรวจสอบได้ จึงไม่สามารถเตรียมตัวอย่างได้</p>}
              <button type="button" className="meeting-intelligence__button" disabled={busy !== null || draft.state !== "private" || !draft.citations.length || timelineIncomplete} onClick={prepareLocalPreview}>
                เตรียมตัวอย่างในเครื่อง
              </button>
            </section>
          )}

          {canDraftLocally && deliveryPreview && (
            <section className="meeting-intelligence__card meeting-intelligence__preview" aria-labelledby="mi-preview-title">
              <h3 id="mi-preview-title">ตัวอย่างการส่งที่ยังอยู่ในเครื่อง</h3>
              <dl>
                <dt>ปลายทาง</dt><dd>{deliveryPreview.destinationSummary}</dd>
                <dt>payload hash</dt><dd><code>{deliveryPreview.payloadHash}</code></dd>
                <dt>สถานะ</dt><dd>{deliveryPreview.state}</dd>
                <dt>ขอบเขตการอนุมัติ</dt><dd>local preview only</dd>
              </dl>
              <p className="meeting-intelligence__warning">ไม่มีการเรียก transport ภายนอกและไม่มีข้อความส่งไปยังห้องประชุม</p>
              {deliveryPreview.state === "awaiting_approval" && (
                <button type="button" className="meeting-intelligence__button meeting-intelligence__button--primary" disabled={busy !== null || timelineIncomplete || MEETING_INTELLIGENCE_EXTERNAL_DISPATCH_AVAILABLE} onClick={approveLocalPreview}>
                  อนุมัติเฉพาะตัวอย่างในเครื่อง
                </button>
              )}
              {deliveryPreview.state === "approved_local_only" && <p className="meeting-intelligence__success">อนุมัติตัวอย่างในเครื่องแล้ว · ยังไม่มีการส่งออก</p>}
            </section>
          )}

          <section className="meeting-intelligence__card" aria-labelledby="mi-history-title">
            <h3 id="mi-history-title">ประวัติการทำงาน</h3>
            {!history.length && <p className="meeting-intelligence__empty">ยังไม่มีรายการ</p>}
            <ol className="meeting-intelligence__history">
              {history.map((entry) => (
                <li key={entry.id}>
                  <strong>{historyLabel(entry)}</strong>
                  <small>{new Date(entry.createdAt).toLocaleString()} · หลักฐาน {entry.evidenceCount} รายการ{entry.transcriptCursor === null ? "" : ` · cursor ${entry.transcriptCursor}`}</small>
                </li>
              ))}
            </ol>
          </section>
        </aside>
      </div>
      <footer className="meeting-intelligence__footer">
        {busy ? `กำลังทำงาน: ${busy}` : loading ? "กำลังอ่านสถานะในเครื่อง…" : "local-only · external provider unavailable"}
      </footer>
    </section>
  );
}

function ReadinessRow({ label, readiness, reason }: { label: string; readiness: string; reason?: string | null }) {
  return (
    <div className="meeting-intelligence__readiness-row">
      <span>{label}</span>
      <span className={`meeting-intelligence__badge meeting-intelligence__badge--${readiness}`}>{readinessLabel(readiness)}{reason ? ` · ${reason}` : ""}</span>
    </div>
  );
}

function formatTime(milliseconds: number): string {
  const totalSeconds = Math.max(0, Math.floor(milliseconds / 1000));
  const minutes = Math.floor(totalSeconds / 60).toString().padStart(2, "0");
  const seconds = (totalSeconds % 60).toString().padStart(2, "0");
  return `${minutes}:${seconds}`;
}

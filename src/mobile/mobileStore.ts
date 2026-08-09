import type { DeviceState, GraphEdge, GraphNode, MobileNote, MobileSnapshot } from "./model";

const STORAGE_KEY = "fung.mobile.snapshot.v1";

const now = () => new Date().toISOString();

const seedNotes: MobileNote[] = [
  {
    id: "note-team-meeting",
    title: "ประชุมทีม",
    body: "สรุปทิศทาง Mobile ให้ทำงานหลักบนอุปกรณ์ และใช้ Desktop เป็นส่วนเสริมสำหรับโมเดลขนาดใหญ่",
    projectId: "project-mobile",
    createdAt: now(),
    updatedAt: now(),
    evidenceLabel: "เสียง 09:42–10:18",
  },
  {
    id: "note-local-first",
    title: "Local-first",
    body: "เสียง โน้ต และกราฟต้องเปิดดูได้แม้ไม่ได้เชื่อมต่อเครือข่าย",
    projectId: "project-mobile",
    createdAt: now(),
    updatedAt: now(),
    evidenceLabel: "ยืนยันโดยผู้ใช้",
  },
  {
    id: "note-desktop-runtime",
    title: "Desktop Runtime",
    body: "ใช้ local LLM ผ่าน LAN เมื่อผู้ใช้อนุญาตเป็นรายโปรเจกต์",
    projectId: "project-mobile",
    createdAt: now(),
    updatedAt: now(),
    evidenceLabel: "ข้อเสนอจากระบบ",
  },
];

const seedNodes: GraphNode[] = [
  { id: "note-team-meeting", label: "ประชุมทีม", kind: "note", x: 50, y: 42 },
  { id: "note-local-first", label: "Local-first", kind: "note", x: 24, y: 68 },
  { id: "note-desktop-runtime", label: "Desktop", kind: "note", x: 76, y: 68 },
  { id: "project-mobile", label: "FUNG Mobile", kind: "project", x: 50, y: 17 },
];

const seedEdges: GraphEdge[] = [
  { id: "edge-1", sourceId: "project-mobile", targetId: "note-team-meeting", predicate: "มีบันทึก", status: "confirmed" },
  { id: "edge-2", sourceId: "note-team-meeting", targetId: "note-local-first", predicate: "ยืนยัน", status: "confirmed" },
  { id: "edge-3", sourceId: "note-team-meeting", targetId: "note-desktop-runtime", predicate: "เกี่ยวข้อง", status: "inferred" },
];

const initialSnapshot = (): MobileSnapshot => ({
  projectId: "project-mobile",
  notes: seedNotes,
  nodes: seedNodes,
  edges: seedEdges,
  devices: [],
});

export function loadSnapshot(): MobileSnapshot {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return JSON.parse(raw) as MobileSnapshot;
  } catch {
    // Corrupt browser preview state is ignored; Tauri persistence remains authoritative.
  }
  const snapshot = initialSnapshot();
  saveSnapshot(snapshot);
  return snapshot;
}

export function saveSnapshot(snapshot: MobileSnapshot): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
}

export function addNote(snapshot: MobileSnapshot, title: string, body: string): MobileSnapshot {
  const timestamp = now();
  const note: MobileNote = {
    id: crypto.randomUUID(),
    title: title.trim() || "โน้ตใหม่",
    body: body.trim(),
    projectId: snapshot.projectId,
    createdAt: timestamp,
    updatedAt: timestamp,
    evidenceLabel: "สร้างบนมือถือ",
  };
  const angle = snapshot.nodes.length * 0.92;
  const node: GraphNode = {
    id: note.id,
    label: note.title,
    kind: "note",
    x: 50 + Math.cos(angle) * 34,
    y: 52 + Math.sin(angle) * 30,
  };
  const edge: GraphEdge = {
    id: crypto.randomUUID(),
    sourceId: snapshot.projectId,
    targetId: note.id,
    predicate: "มีบันทึก",
    status: "confirmed",
  };
  const next = { ...snapshot, notes: [note, ...snapshot.notes], nodes: [...snapshot.nodes, node], edges: [...snapshot.edges, edge] };
  saveSnapshot(next);
  return next;
}

export function upsertPairedDevice(
  snapshot: MobileSnapshot,
  device: { cloudDeviceId: string; name: string; endpoint: string; pairingSessionId: string },
): MobileSnapshot {
  const rest = snapshot.devices.filter((d) => d.cloudDeviceId !== device.cloudDeviceId);
  const entry: DeviceState = {
    id: device.cloudDeviceId,
    cloudDeviceId: device.cloudDeviceId,
    name: device.name,
    endpoint: device.endpoint,
    trustState: "paired",
    capabilities: [],
    pairingSessionId: device.pairingSessionId,
    lastSeenAt: now(),
  };
  const next = { ...snapshot, devices: [...rest, entry] };
  saveSnapshot(next);
  return next;
}

// Genesis's `paired_devices.trust_state` (written by the Rust fungwire
// client on every reachability probe / delegate handshake) and this
// mobileStore snapshot's `trustState` are two different stores that started
// out in sync at pairing time but otherwise never talk to each other —
// nothing previously kept the snapshot's flag in step with reality. Callers
// probe live via `desktopReachable(...)` (see MobileApp.tsx) and report the
// result here so the Devices screen chip and any other reader of
// `trustState` reflect the ACTUAL probe outcome rather than a stale flag
// frozen at pairing time. Mirrors `mark_peer_unreachable`/
// `mark_peer_reachable` in `fungwire_client.rs`: a `"revoked"` device is
// never resurrected by a reachability result, and a no-op transition
// returns the same snapshot reference so callers can skip re-rendering.
export function setDeviceReachability(snapshot: MobileSnapshot, cloudDeviceId: string, reachable: boolean): MobileSnapshot {
  let changed = false;
  const devices = snapshot.devices.map((device) => {
    if (device.cloudDeviceId !== cloudDeviceId || device.trustState === "revoked") return device;
    const trustState: DeviceState["trustState"] = reachable ? "paired" : "unreachable";
    if (device.trustState === trustState) return device;
    changed = true;
    return { ...device, trustState, lastSeenAt: reachable ? now() : device.lastSeenAt };
  });
  if (!changed) return snapshot;
  const next = { ...snapshot, devices };
  saveSnapshot(next);
  return next;
}

export function markDeviceRevoked(snapshot: MobileSnapshot, cloudDeviceId: string): MobileSnapshot {
  const next = {
    ...snapshot,
    devices: snapshot.devices.map((d) =>
      d.cloudDeviceId === cloudDeviceId ? { ...d, trustState: "revoked" as const } : d,
    ),
  };
  saveSnapshot(next);
  return next;
}

export function removeDevice(snapshot: MobileSnapshot, id: string): MobileSnapshot {
  const next = { ...snapshot, devices: snapshot.devices.filter((device) => device.id !== id) };
  saveSnapshot(next);
  return next;
}

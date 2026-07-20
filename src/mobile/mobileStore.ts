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

export function pairDevice(snapshot: MobileSnapshot, name: string, endpoint: string): MobileSnapshot {
  const device: DeviceState = {
    id: crypto.randomUUID(),
    name: name.trim() || "FUNG Desktop",
    endpoint: endpoint.trim() || "ค้นหาในเครือข่ายภายใน",
    trustState: "paired",
    capabilities: ["local-llm", "transcription", "mcp"],
    lastSeenAt: now(),
  };
  const next = { ...snapshot, devices: [device, ...snapshot.devices.filter((item) => item.endpoint !== device.endpoint)] };
  saveSnapshot(next);
  return next;
}

export function removeDevice(snapshot: MobileSnapshot, id: string): MobileSnapshot {
  const next = { ...snapshot, devices: snapshot.devices.filter((device) => device.id !== id) };
  saveSnapshot(next);
  return next;
}

import type { DeviceState, GraphEdge, GraphNode, MobileNote, MobileSnapshot } from "./model";

const STORAGE_KEY = "fung.mobile.snapshot.v1";

const now = () => new Date().toISOString();

// The store seeds only the real project node. Earlier builds seeded three
// invented notes ("ประชุมทีม เสียง 09:42–10:18" and friends) plus edges, which
// rendered as if the user had recorded a meeting that never happened — the
// same fabricated-data defect the desktop shell shipped with. Real content
// comes from the user and from Genesis; an empty store stays visibly empty.
const LEGACY_SEED_IDS = new Set([
  "note-team-meeting",
  "note-local-first",
  "note-desktop-runtime",
  "edge-1",
  "edge-2",
  "edge-3",
]);

const seedNodes: GraphNode[] = [
  { id: "project-mobile", label: "FUNG Mobile", kind: "project", x: 50, y: 17 },
];

const initialSnapshot = (): MobileSnapshot => ({
  projectId: "project-mobile",
  notes: [],
  nodes: seedNodes,
  edges: [],
  devices: [],
});

/** Drops the fabricated seed rows an older build may have persisted. */
function purgeLegacySeeds(snapshot: MobileSnapshot): MobileSnapshot {
  const notes = snapshot.notes.filter((note) => !LEGACY_SEED_IDS.has(note.id));
  const nodes = snapshot.nodes.filter((node) => !LEGACY_SEED_IDS.has(node.id));
  const edges = snapshot.edges.filter(
    (edge) =>
      !LEGACY_SEED_IDS.has(edge.id) &&
      !LEGACY_SEED_IDS.has(edge.sourceId) &&
      !LEGACY_SEED_IDS.has(edge.targetId),
  );
  if (
    notes.length === snapshot.notes.length &&
    nodes.length === snapshot.nodes.length &&
    edges.length === snapshot.edges.length
  ) {
    return snapshot;
  }
  const next = { ...snapshot, notes, nodes, edges };
  saveSnapshot(next);
  return next;
}

export function loadSnapshot(): MobileSnapshot {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) return purgeLegacySeeds(JSON.parse(raw) as MobileSnapshot);
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

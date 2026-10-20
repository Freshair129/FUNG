export function resolveRunStatus(receipt, nowMs, staleAfterMs) {
  if (receipt.status !== 'RUNNING') return receipt.status;
  const startedAtMs = Date.parse(receipt.startedAt ?? '');
  if (!Number.isFinite(startedAtMs) || nowMs - startedAtMs <= staleAfterMs) return 'RUNNING';
  return 'STALE';
}

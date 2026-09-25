import { useCallback, useEffect, useRef, useState } from "react";
import type { FormEvent } from "react";
import type {
  MeetingIntelligenceService,
  MeetingPeopleLink,
  MeetingPeopleProfile,
  MeetingPeopleSnapshot,
  NativeMeetingScope,
} from "../lib/meetingIntelligence.ts";

type Props = {
  projectId: string;
  scope: NativeMeetingScope | null;
  service: MeetingIntelligenceService;
  enabled: boolean;
  accountLifecycleRevision: number;
  accountLifecycleRef: { current: number };
};

function linkStatusLabel(link: MeetingPeopleLink): string {
  if (link.status === "stale") return "หลักฐานหรือโปรไฟล์เปลี่ยนแล้ว · ตรวจทานใหม่";
  if (link.status === "pending_review") return "รอตรวจทาน";
  if (link.status === "confirmed") return "ยืนยันแล้ว";
  if (link.status === "rejected") return "ปฏิเสธแล้ว";
  return "ถอนการเชื่อมแล้ว";
}

export function MeetingPeoplePanel({ projectId, scope, service, enabled, accountLifecycleRevision, accountLifecycleRef }: Props) {
  const [snapshot, setSnapshot] = useState<MeetingPeopleSnapshot | null>(null);
  const [newName, setNewName] = useState("");
  const [speakerId, setSpeakerId] = useState("");
  const [profileId, setProfileId] = useState("");
  const [editingProfileId, setEditingProfileId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const currentSnapshot = enabled ? snapshot : null;

  const refresh = useCallback(async () => {
    if (!scope || !enabled) {
      setSnapshot(null);
      return;
    }
    const accountLifecycle = accountLifecycleRevision;
    const result = await service.people(scope);
    if (accountLifecycleRef.current !== accountLifecycle) return;
    setSnapshot(result);
    setSpeakerId((current) => current && result.speakers.some((row) => row.speakerId === current)
      ? current
      : result.speakers[0]?.speakerId ?? "");
    setProfileId((current) => current && result.profiles.some((row) => row.profileId === current)
      ? current
      : result.profiles[0]?.profileId ?? "");
  }, [accountLifecycleRef, accountLifecycleRevision, enabled, scope, service]);

  useEffect(() => {
    let cancelled = false;
    if (!enabled || !scope) {
      setSnapshot(null);
      setNewName("");
      setEditingProfileId(null);
      setEditingName("");
      setBusy(false);
      setError(null);
      return () => { cancelled = true; };
    }
    void service.people(scope).then((result) => {
      if (!cancelled && accountLifecycleRef.current === accountLifecycleRevision) {
        setSnapshot(result);
        setSpeakerId((current) => current && result.speakers.some((row) => row.speakerId === current)
          ? current
          : result.speakers[0]?.speakerId ?? "");
        setProfileId((current) => current && result.profiles.some((row) => row.profileId === current)
          ? current
          : result.profiles[0]?.profileId ?? "");
      }
    }).catch(() => {
      if (!cancelled && accountLifecycleRef.current === accountLifecycleRevision) setError("อ่าน local People vault ไม่ได้ กรุณาตรวจสถานะการปลดล็อก");
    });
    return () => { cancelled = true; };
  }, [accountLifecycleRef, accountLifecycleRevision, enabled, scope, service]);

  const run = useCallback(async (operation: () => Promise<unknown>) => {
    if (busy) return;
    const accountLifecycle = accountLifecycleRevision;
    setBusy(true);
    setError(null);
    try {
      await operation();
      if (accountLifecycleRef.current !== accountLifecycle) return;
      await refresh();
    } catch {
      if (accountLifecycleRef.current === accountLifecycle) setError("รายการ People ไม่สำเร็จ อาจมีการแก้ revision หรือหลักฐาน transcript เปลี่ยนแล้ว");
    } finally {
      setBusy(false);
    }
  }, [accountLifecycleRef, accountLifecycleRevision, busy, refresh]);

  if (!scope) {
    return (
      <section className="meeting-intelligence__card meeting-people" aria-labelledby="mi-people-title">
        <h3 id="mi-people-title">People และการตรวจทานผู้พูด</h3>
        <p className="meeting-intelligence__empty">ต้องมี transcript revision ในขอบเขตประชุมนี้ก่อน จึงจะตรวจทานผู้พูดได้</p>
      </section>
    );
  }

  const createProfile = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const displayName = newName.trim();
    if (!displayName || !enabled) return;
    void run(async () => {
      await service.createPersonProfile(projectId, displayName);
      setNewName("");
    });
  };

  const saveProfile = (profile: MeetingPeopleProfile) => {
    const displayName = editingName.trim();
    if (!displayName || !enabled) return;
    void run(async () => {
      await service.updatePersonProfile(projectId, profile.profileId, profile.revision, displayName);
      setEditingProfileId(null);
      setEditingName("");
    });
  };

  const proposeLink = () => {
    const speaker = currentSnapshot?.speakers.find((row) => row.speakerId === speakerId);
    if (!speaker || !profileId || !enabled) return;
    void run(() => service.proposeSpeakerIdentity({
      scope,
      speakerId,
      profileId,
      expectedEvidenceRevision: speaker.evidenceRevision,
    }));
  };

  const mutateLink = (link: MeetingPeopleLink, action: "confirm" | "reject" | "unlink") => {
    if (!enabled) return;
    const request = { scope, linkId: link.linkId, expectedRevision: link.revision };
    const operation = action === "confirm"
      ? () => service.confirmSpeakerIdentity(request)
      : action === "reject"
        ? () => service.rejectSpeakerIdentity(request)
        : () => service.unlinkSpeakerIdentity(request);
    void run(operation);
  };

  return (
    <section className="meeting-intelligence__card meeting-people" aria-labelledby="mi-people-title">
      <h3 id="mi-people-title">People และการตรวจทานผู้พูด</h3>
      <p className="meeting-intelligence__muted">ชื่อจัดเก็บใน vault ที่เข้ารหัส การยืนยันเป็นการตรวจทานด้วยมือและผูกกับ revision ของ transcript</p>
      {!enabled && <p className="meeting-intelligence__warning">ปลดล็อก local owner vault ผ่านแผงนี้เพื่อจัดการ People</p>}
      {error && <p className="meeting-intelligence__error" role="alert">{error}</p>}

      <form className="meeting-people__create" onSubmit={createProfile}>
        <label className="meeting-intelligence__field">
          เพิ่มชื่อบุคคลใน vault
          <input value={newName} maxLength={256} disabled={!enabled || busy} onChange={(event) => setNewName(event.target.value)} />
        </label>
        <button type="submit" className="meeting-intelligence__button" disabled={!enabled || busy || !newName.trim()}>เพิ่มโปรไฟล์</button>
      </form>

      <ul className="meeting-people__profiles">
        {!currentSnapshot?.profiles.length && <li className="meeting-intelligence__empty">ยังไม่มีโปรไฟล์ People</li>}
        {currentSnapshot?.profiles.map((profile) => (
          <li key={profile.profileId}>
            {editingProfileId === profile.profileId ? (
              <>
                <label className="meeting-intelligence__field">
                  แก้ชื่อบุคคล
                  <input value={editingName} maxLength={256} disabled={busy} onChange={(event) => setEditingName(event.target.value)} />
                </label>
                <button type="button" className="meeting-intelligence__button" disabled={busy || !editingName.trim()} onClick={() => saveProfile(profile)}>บันทึก</button>
                <button type="button" className="meeting-intelligence__button" disabled={busy} onClick={() => setEditingProfileId(null)}>ยกเลิก</button>
              </>
            ) : (
              <>
                <span><strong>{profile.displayName}</strong><small>rev {profile.revision}</small></span>
                <button type="button" className="meeting-intelligence__text-button" disabled={!enabled || busy} onClick={() => { setEditingProfileId(profile.profileId); setEditingName(profile.displayName); }}>แก้ชื่อ</button>
                <button type="button" className="meeting-intelligence__text-button" disabled={!enabled || busy} onClick={() => void run(() => service.archivePersonProfile(profile.profileId, profile.revision))}>เก็บเข้าคลัง</button>
              </>
            )}
          </li>
        ))}
      </ul>

      <div className="meeting-people__proposal">
        <label className="meeting-intelligence__field">
          ผู้พูดจากหลักฐานปัจจุบัน
          <select value={speakerId} disabled={!enabled || busy || !currentSnapshot?.speakers.length} onChange={(event) => setSpeakerId(event.target.value)}>
            {!currentSnapshot?.speakers.length && <option value="">ไม่มี speaker attribution</option>}
            {currentSnapshot?.speakers.map((speaker) => <option key={speaker.speakerId} value={speaker.speakerId}>{speaker.displayLabel} · rev {speaker.evidenceRevision}</option>)}
          </select>
        </label>
        <label className="meeting-intelligence__field">
          โปรไฟล์ที่ต้องการตรวจทาน
          <select value={profileId} disabled={!enabled || busy || !currentSnapshot?.profiles.length} onChange={(event) => setProfileId(event.target.value)}>
            {!currentSnapshot?.profiles.length && <option value="">เพิ่มโปรไฟล์ก่อน</option>}
            {currentSnapshot?.profiles.map((profile) => <option key={profile.profileId} value={profile.profileId}>{profile.displayName}</option>)}
          </select>
        </label>
        <button type="button" className="meeting-intelligence__button meeting-intelligence__button--primary" disabled={!enabled || busy || !speakerId || !profileId} onClick={proposeLink}>เสนอให้ตรวจทาน</button>
      </div>

      <ul className="meeting-people__links">
        {!currentSnapshot?.links.length && <li className="meeting-intelligence__empty">ยังไม่มี speaker link ใน recording นี้</li>}
        {currentSnapshot?.links.map((link) => (
          <li key={link.linkId}>
            <span><strong>{link.displayName}</strong><small>{linkStatusLabel(link)} · evidence rev {link.evidenceRevision} · link rev {link.revision}</small></span>
            {link.status === "pending_review" && (
              <div className="meeting-intelligence__actions">
                <button type="button" className="meeting-intelligence__button" disabled={!enabled || busy} onClick={() => mutateLink(link, "confirm")}>ยืนยัน</button>
                <button type="button" className="meeting-intelligence__button" disabled={!enabled || busy} onClick={() => mutateLink(link, "reject")}>ปฏิเสธ</button>
              </div>
            )}
            {link.status === "stale" && (
              <button type="button" className="meeting-intelligence__button" disabled={!enabled || busy} onClick={() => mutateLink(link, "reject")}>ปิดรายการเก่า</button>
            )}
            {link.status === "confirmed" && (
              <button type="button" className="meeting-intelligence__button" disabled={!enabled || busy} onClick={() => mutateLink(link, "unlink")}>ถอนการเชื่อม</button>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

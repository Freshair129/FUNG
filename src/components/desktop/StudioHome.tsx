import { ArrowRight, AudioLines, CalendarDays, Circle, ClipboardList, Clock3, FolderOpen, Headphones, MessageSquareText, Mic, Play, ShieldCheck, Upload, Users } from "lucide-react";
import type { RefObject } from "react";
import type { DesktopMeetingActionItem } from "../../tauri.ts";
import type { DesktopShellProps, DesktopSurface, MeetingActionItemsLoadState } from "./contracts";

type StudioHomeProps = Pick<DesktopShellProps, "project" | "selectedProjectId" | "liveStatus"> & {
  headingRef: RefObject<HTMLHeadingElement>;
  captureActive: boolean;
  actionItems: MeetingActionItemsLoadState;
  onNavigate: (surface: DesktopSurface, initiator: HTMLElement) => void;
  onOpenActionItem: (item: DesktopMeetingActionItem, initiator: HTMLElement) => void;
  onImport: () => void;
  onProjects: () => void;
};

const meetingDateLabel = new Intl.DateTimeFormat("th-TH", { day: "numeric", month: "short" });

function sourceDateLabel(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? "วันที่ไม่ระบุ" : meetingDateLabel.format(date);
}

function priorityLabel(priority: DesktopMeetingActionItem["priority"]): string {
  if (priority === "urgent") return "เร่งด่วน";
  if (priority === "deadline") return "มีกำหนดส่ง";
  return "ไม่ระบุลำดับ";
}

export function StudioHome({ project, selectedProjectId, liveStatus, headingRef, captureActive, actionItems, onNavigate, onOpenActionItem, onImport, onProjects }: StudioHomeProps) {
  const projects = project.data ?? [];
  const available = project.status === "ready" && Array.isArray(project.data);
  const selectedProject = projects.find((item) => item.id === selectedProjectId);
  const liveKnown = liveStatus.status === "ready" && liveStatus.data !== null;
  const liveLabel = captureActive ? "กำลังบันทึก" : liveKnown ? "พร้อมเริ่มงาน"
    : liveStatus.status === "error" ? "อ่านสถานะการบันทึกไม่สำเร็จ"
      : liveStatus.status === "unavailable" ? "สถานะการบันทึกยังไม่พร้อมอ่าน" : "กำลังอ่านสถานะ";
  const actionItemsList = actionItems.data?.items ?? [];
  const actionItemsTotal = actionItems.data?.totalCount ?? 0;

  return (
    <div className="studio-home" aria-label="ภาพรวมพื้นที่ทำงาน">
      <section className="studio-hero" aria-labelledby="desktop-shell-main-heading">
        <img className="studio-hero__image" src="/images/fung-listening-hero.png" alt="" />
        <div className="studio-hero__top">
          <span><Headphones size={16} /> พื้นที่ของคุณ</span>
          <button type="button" onClick={onProjects}><FolderOpen size={16} /> เลือกโครงการ</button>
        </div>
        <div className="studio-hero__story">
          <span className="studio-hero__tag">YOUR QUIET SPACE</span>
          <h1 id="desktop-shell-main-heading" ref={headingRef} tabIndex={-1}>ฟังทุกเรื่องสำคัญ</h1>
          <p>บันทึกบทสนทนา เก็บไอเดีย<br />แล้วกลับมาฟังสิ่งสำคัญได้เสมอ</p>
          <div className="studio-hero__facts">
            <span><FolderOpen size={19} /><span><strong>{available ? projects.length.toLocaleString("th-TH") : "—"}</strong><small>โครงการ</small></span></span>
            <span><ShieldCheck size={19} /><span><strong>Local first</strong><small>พื้นที่ส่วนตัว</small></span></span>
            <span><AudioLines size={19} /><span><strong>{liveLabel}</strong><small>สถานะการบันทึก</small></span></span>
          </div>
          <button className="studio-hero__start" type="button" onClick={(event) => onNavigate("live", event.currentTarget)} aria-describedby="desktop-shell-live-action-help"><Play size={16} fill="currentColor" /> เริ่มประชุม</button>
          <small id="desktop-shell-live-action-help" className="studio-hero__help">เปิดพื้นที่ประชุมก่อนเริ่มบันทึกเสียง</small>
        </div>
        <div className="studio-hero__shortcuts">
          <div className="studio-section-heading"><h2>ไปต่อกับงานของคุณ</h2><button type="button" onClick={(event) => onNavigate("review", event.currentTarget)}>ดูบันทึก <ArrowRight size={13} /></button></div>
          <div className="studio-shortcuts">
            <button type="button" onClick={(event) => onNavigate("live", event.currentTarget)}><Mic size={18} /><span>ประชุมสด</span><small>บันทึกบทสนทนา</small></button>
            <button type="button" onClick={onImport}><Upload size={18} /><span>นำเข้าเสียง</span><small>ไฟล์เสียง / วิดีโอ</small></button>
            <button type="button" onClick={(event) => onNavigate("review", event.currentTarget)}><Headphones size={18} /><span>ย้อนฟัง</span><small>ทบทวนบันทึก</small></button>
          </div>
        </div>
      </section>

      <section className="studio-meeting studio-card" aria-labelledby="studio-meeting-heading">
        <div className="studio-meeting__header">
          <h2 id="studio-meeting-heading">การประชุมครั้งถัดไป</h2>
          <span className="studio-meeting__status">ยังไม่มีข้อมูลปฏิทิน</span>
        </div>
        <div className="studio-meeting__empty">
          <CalendarDays size={27} aria-hidden="true" />
          <strong>ยังไม่มีข้อมูลกำหนดการ</strong>
          <span>รายละเอียดนัดจะแสดงเมื่อมีข้อมูลปฏิทิน</span>
        </div>
        <dl className="studio-meeting__details">
          <div><dt><Clock3 size={14} /> วันและเวลา</dt><dd>—</dd></div>
          <div><dt><Users size={14} /> ผู้เข้าร่วม</dt><dd>—</dd></div>
          <div><dt><MessageSquareText size={14} /> หัวข้อ</dt><dd>—</dd></div>
        </dl>
        <button className="studio-meeting__action" type="button" onClick={(event) => onNavigate("live", event.currentTarget)}><Mic size={15} /> เริ่มประชุมสด <ArrowRight size={13} /></button>
      </section>

      <section className="studio-activity studio-card" aria-labelledby="studio-activity-heading">
        <h2 id="studio-activity-heading">พื้นที่ในเครื่อง</h2>
        {liveStatus.status === "error" ? <p className="studio-activity__notice" role="alert">อ่านสถานะการบันทึกไม่สำเร็จ</p> : null}
        <div className="studio-activity__count">
          <Circle size={142} strokeWidth={1.2} aria-hidden="true" />
          <div><FolderOpen size={20} /><strong>{available ? projects.length.toLocaleString("th-TH") : "—"}</strong><small>โครงการทั้งหมด</small></div>
        </div>
        <dl>
          <div><dt><AudioLines size={13} /> สถานะ</dt><dd>{liveLabel}</dd></div>
          <div><dt><FolderOpen size={13} /> โครงการ</dt><dd><button type="button" onClick={onProjects} title={selectedProject?.name}>{selectedProject?.name ?? "เลือกโครงการ"}</button></dd></div>
          <div><dt><ShieldCheck size={13} /> การจัดเก็บ</dt><dd>ในเครื่องของคุณ</dd></div>
        </dl>
      </section>

      <section className="studio-task-pool studio-card" aria-labelledby="studio-task-pool-heading">
        <div className="studio-task-pool__header">
          <div>
            <h2 id="studio-task-pool-heading">งานจากทุกการประชุม</h2>
            <p>AI สกัดและเรียงตามคำเร่งด่วนหรือกำหนดส่ง</p>
          </div>
          <span className="studio-task-pool__count">{actionItems.status === "ready" ? actionItemsTotal.toLocaleString("th-TH") : "—"} รายการ</span>
        </div>

        <div className="studio-task-pool__list" aria-busy={actionItems.status === "loading"}>
          {actionItems.status === "loading" ? <p className="studio-task-pool__empty"><ClipboardList size={25} />กำลังอ่านรายการงานจากประชุม</p> : null}
          {actionItems.status === "unavailable" ? <p className="studio-task-pool__empty"><ClipboardList size={25} />เปิด FUNG Desktop เพื่ออ่านรายการงานในเครื่อง</p> : null}
          {actionItems.status === "error" ? <p className="studio-task-pool__empty" role="alert"><ClipboardList size={25} />{actionItems.error ?? "อ่านรายการงานไม่สำเร็จ"}</p> : null}
          {actionItems.status === "ready" && actionItemsList.length === 0 ? <p className="studio-task-pool__empty"><ClipboardList size={25} />ยังไม่มีงานที่ AI สกัดจากการประชุม</p> : null}
          {actionItems.status === "ready" && actionItemsList.length > 0 ? (
            <ol className="studio-task-pool__items">
              {actionItemsList.slice(0, 4).map((item) => (
                <li key={item.id}>
                  <button type="button" className="studio-task" onClick={(event) => onOpenActionItem(item, event.currentTarget)} aria-label={`เปิดบันทึกต้นทางของงาน: ${item.title}`}>
                    <span className={`studio-task__priority studio-task__priority--${item.priority}`}>{priorityLabel(item.priority)}</span>
                    <strong>{item.title}</strong>
                    <span className="studio-task__source">{item.meetingTitle} · {item.projectName} · {sourceDateLabel(item.meetingCreatedAt)}</span>
                    <span className="studio-task__evidence">{item.owner ? `ผู้รับผิดชอบ ${item.owner}` : "ยังไม่ระบุผู้รับผิดชอบ"} · หลักฐาน {item.evidenceCount} ช่วงเสียง</span>
                  </button>
                </li>
              ))}
            </ol>
          ) : null}
        </div>

        <div className="studio-task-pool__footer"><ShieldCheck size={13} /><span>รายการจาก AI เป็นข้อเสนอ ตรวจทานก่อนถือเป็นงานยืนยัน</span><button type="button" onClick={(event) => onNavigate("review", event.currentTarget)}>ดูบันทึก <ArrowRight size={12} /></button></div>
      </section>
    </div>
  );
}

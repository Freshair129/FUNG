import { useState, type RefObject } from "react";
import { ArrowDownToLine, ArrowRight, AudioLines, CalendarDays, Circle, Clock3, FolderOpen, Headphones, Mic, Play, ShieldCheck, Sparkles, Upload } from "lucide-react";
import type { DesktopShellProps, DesktopSurface } from "./contracts";

type StudioHomeProps = Pick<DesktopShellProps, "project" | "selectedProjectId" | "liveStatus"> & {
  headingRef: RefObject<HTMLHeadingElement>;
  captureActive: boolean;
  onNavigate: (surface: DesktopSurface, initiator: HTMLElement) => void;
  onImport: () => void;
  onProjects: () => void;
  onCompanion: () => void;
};

const dayLabel = new Intl.DateTimeFormat("th-TH", { day: "numeric", month: "short" });

export function StudioHome({ project, selectedProjectId, liveStatus, headingRef, captureActive, onNavigate, onImport, onProjects, onCompanion }: StudioHomeProps) {
  const [period, setPeriod] = useState(7);
  const projects = project.data ?? [];
  const available = project.status === "ready" && Array.isArray(project.data);
  const selectedProject = projects.find((item) => item.id === selectedProjectId);
  const now = new Date();
  const dateKey = (date: Date) => `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
  const projectCounts = new Map<string, number>();
  for (const item of projects) {
    const created = new Date(item.createdAt);
    if (Number.isNaN(created.getTime())) continue;
    const key = dateKey(created);
    projectCounts.set(key, (projectCounts.get(key) ?? 0) + 1);
  }
  const days = Array.from({ length: period }, (_, index) => {
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - period + index + 1);
    return { date: start, count: projectCounts.get(dateKey(start)) ?? 0 };
  });
  const total = days.reduce((sum, day) => sum + day.count, 0);
  const maximum = Math.max(1, ...days.map((day) => day.count));
  const liveKnown = liveStatus.status === "ready" && liveStatus.data !== null;
  const liveLabel = captureActive ? "กำลังบันทึก" : liveKnown ? "พร้อมเริ่มงาน"
    : liveStatus.status === "error" ? "อ่านสถานะการบันทึกไม่สำเร็จ"
      : liveStatus.status === "unavailable" ? "สถานะการบันทึกยังไม่พร้อมอ่าน" : "กำลังอ่านสถานะ";
  const activityMessage = project.status === "loading" || project.status === "idle"
    ? "กำลังอ่านกิจกรรมในเครื่อง"
    : project.status === "error"
      ? "อ่านกิจกรรมไม่สำเร็จ ลองเปิดโครงการอีกครั้ง"
      : !available ? "เปิด FUNG Desktop เพื่อดูกิจกรรมของคุณ"
        : total === 0 ? "ยังไม่มีโครงการใหม่ในช่วงนี้" : null;

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
            <button type="button" onClick={(event) => onNavigate("output", event.currentTarget)}><ArrowDownToLine size={18} /><span>ไฟล์บันทึก</span><small>จัดการพื้นที่เก็บ</small></button>
          </div>
        </div>
      </section>

      <section className="studio-assistant studio-card" aria-labelledby="studio-assistant-heading">
        <h2 id="studio-assistant-heading">AI Companion</h2>
        <img src="/images/fung-assistant-waveform.png" className="studio-assistant__wave" alt="" />
        <button type="button" className="studio-assistant__button" onClick={onCompanion} aria-label="เปิดผู้ช่วย Companion"><Sparkles size={23} /></button>
        <strong>ผู้ช่วยของคุณ</strong>
        <p>เปิดทางลัดและดูบริบทงาน</p>
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

      <section className="studio-progress studio-card" aria-labelledby="studio-progress-heading">
        <div className="studio-section-heading">
          <h2 id="studio-progress-heading">กิจกรรมโครงการ</h2>
          <select aria-label="ช่วงเวลากิจกรรม" value={period} onChange={(event) => setPeriod(Number(event.target.value))}><option value={7}>7 วันล่าสุด</option><option value={30}>30 วันล่าสุด</option></select>
        </div>
        <div className="studio-progress__chart" aria-label="จำนวนโครงการที่สร้างในแต่ละวัน">
          {activityMessage ? <p className="studio-progress__empty"><CalendarDays size={26} />{activityMessage}</p> : (
            <div className="studio-progress__bars">
              {days.map((day) => <div key={day.date.toISOString()}><meter min={0} max={maximum} value={day.count} aria-label={`${dayLabel.format(day.date)}: ${day.count} โครงการ`} title={`${dayLabel.format(day.date)} · ${day.count} โครงการ`} /><span>{period === 7 ? dayLabel.format(day.date) : day.date.getDate()}</span></div>)}
            </div>
          )}
        </div>
        <p className="studio-progress__legend"><FolderOpen size={12} /> โครงการที่สร้างใหม่ <span>ข้อมูลจากเครื่องนี้</span></p>
        <div className="studio-progress__stats">
          <div><CalendarDays size={17} /><strong>{available ? total : "—"}</strong><span>โครงการใหม่<br />ในช่วงนี้</span></div>
          <div><Clock3 size={17} /><strong>{period}</strong><span>วัน<br />ที่แสดง</span></div>
          <div><FolderOpen size={17} /><strong>{available ? projects.length : "—"}</strong><span>โครงการ<br />ทั้งหมด</span></div>
        </div>
        <div className="studio-progress__footer"><ShieldCheck size={13} /><span>ค่อย ๆ เก็บทุกบทสนทนาที่สำคัญ</span><button type="button" onClick={onProjects}>เปิดโครงการ <ArrowRight size={12} /></button></div>
      </section>
    </div>
  );
}

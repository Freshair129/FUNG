import { useEffect } from "react";
import {
  ArrowDown,
  ArrowUpRight,
  AudioLines,
  Cloud,
  Database,
  FileText,
  Fingerprint,
  LockKeyhole,
  Network,
} from "lucide-react";
import ribbonAsset from "./assets/porcelain-memory-ribbon.png";
import "./landing.css";

const waveform = Array.from({ length: 54 }, (_, index) => {
  const center = Math.abs(index - 27) / 27;
  return Math.round(18 + (1 - center) * 36 + Math.sin(index * 1.73) * 15);
});

const graphNodes = [
  { label: "ความตั้งใจ", x: 50, y: 48, primary: true },
  { label: "ความเงียบ", x: 24, y: 22 },
  { label: "สิ่งที่ไม่ได้พูด", x: 76, y: 22 },
  { label: "บริบทก่อนหน้า", x: 18, y: 72 },
  { label: "สิ่งที่ต้องการ", x: 79, y: 73 },
];

function ArrowIcon() {
  return <ArrowUpRight aria-hidden="true" size={17} strokeWidth={1.6} />;
}

function useScrollNarrative() {
  useEffect(() => {
    const root = document.documentElement;
    const sections = Array.from(document.querySelectorAll<HTMLElement>("[data-scroll-scene]"));
    let frame = 0;

    const update = () => {
      const viewport = window.innerHeight;
      const pageRange = Math.max(document.documentElement.scrollHeight - viewport, 1);
      root.style.setProperty("--landing-scroll", String(window.scrollY / pageRange));

      sections.forEach((section) => {
        const rect = section.getBoundingClientRect();
        const range = rect.height + viewport;
        const progress = Math.min(1, Math.max(0, (viewport - rect.top) / range));
        section.style.setProperty("--progress", progress.toFixed(4));
      });
      frame = 0;
    };

    const requestUpdate = () => {
      if (!frame) frame = window.requestAnimationFrame(update);
    };

    update();
    window.addEventListener("scroll", requestUpdate, { passive: true });
    window.addEventListener("resize", requestUpdate);
    return () => {
      window.removeEventListener("scroll", requestUpdate);
      window.removeEventListener("resize", requestUpdate);
      if (frame) window.cancelAnimationFrame(frame);
      root.style.removeProperty("--landing-scroll");
    };
  }, []);
}

export function LandingPage() {
  useScrollNarrative();

  useEffect(() => {
    const previousTitle = document.title;
    document.title = "FUNG — ฟังสิ่งที่ไม่ได้พูด";
    return () => {
      document.title = previousTitle;
    };
  }, []);

  return (
    <main className="landing-page">
      <div className="landing-progress" aria-hidden="true" />

      <header className="landing-header">
        <a className="landing-wordmark" href="#top" aria-label="FUNG หน้าแรก">
          FUNG
        </a>
        <nav className="landing-nav" aria-label="เมนูหลัก">
          <a href="#product">Product</a>
          <a href="#how-it-works">How it works</a>
          <a href="#privacy">Privacy</a>
        </nav>
        <a className="landing-header-cta" href="/app">
          เปิด FUNG <ArrowIcon />
        </a>
      </header>

      <section id="top" className="hero-scene" data-scroll-scene>
        <div className="hero-sticky">
          <img className="hero-ribbon" src={ribbonAsset} alt="" aria-hidden="true" />
          <div className="hero-copy">
            <h1>
              <span><strong>FUNG</strong> ในสิ่งที่เค้าไม่ได้พูด</span>
              <span>แล้วเราจะได้ยินสิ่งที่เค้าต้องการ</span>
            </h1>
            <p>
              จับบริบทจากเสียง ความเงียบ และสิ่งที่เชื่อมโยงกัน
              <br />โดยข้อมูลหลักยังอยู่บนอุปกรณ์ของคุณ
            </p>
            <div className="hero-actions">
              <a className="landing-button landing-button-primary" href="/app">
                เปิด FUNG <ArrowIcon />
              </a>
              <a className="landing-button landing-button-secondary" href="#how-it-works">
                ดูวิธีทำงาน <ArrowDown aria-hidden="true" size={17} strokeWidth={1.6} />
              </a>
            </div>
          </div>

          <div className="hero-signal hero-signal-a" aria-hidden="true">
            <span>ช่วงเงียบ 2.4 วินาที</span>
            <div className="signal-dash" />
          </div>
          <div className="hero-signal hero-signal-b" aria-hidden="true">
            <small>00:08:11</small>
            <p>ถ้าเราย้อนกลับไปดู<br />จะเห็นสิ่งที่เชื่อมโยงกัน</p>
            <div className="mini-wave">
              {waveform.slice(5, 26).map((height, index) => (
                <i key={index} style={{ height: `${Math.max(8, height * 0.35)}px` }} />
              ))}
            </div>
          </div>
          <div className="hero-next-preview" aria-hidden="true">
            <small>01</small>
            <strong>บันทึกความคิด<br />ด้วยเสียงของคุณ</strong>
            <div>
              <time>00:27</time>
              <span className="preview-wave">
                {waveform.slice(8, 40).map((height, index) => (
                  <i key={index} style={{ height: `${Math.max(8, height * 0.45)}px` }} />
                ))}
              </span>
            </div>
          </div>
          <p className="hero-scroll-cue" aria-hidden="true">
            Scroll to listen <ArrowDown size={15} />
          </p>
        </div>
      </section>

      <section id="how-it-works" className="knowledge-scene" data-scroll-scene>
        <div className="knowledge-sticky">
          <div className="section-index" aria-hidden="true">02 / 04</div>
          <div className="knowledge-heading">
            <h2>จากเสียงหนึ่งช่วงเวลา<br />สู่ความรู้ที่ย้อนกลับไป<br />หาแหล่งจริงได้</h2>
            <p>FUNG ไม่ได้หยุดอยู่แค่การฟัง แต่รักษาเส้นทางจากข้อสังเกตกลับไปยังเสียงต้นทางเสมอ</p>
          </div>

          <div className="knowledge-ribbon" aria-hidden="true" />

          <article className="story-step story-capture">
            <div className="story-label"><b>01</b><span>บันทึก</span></div>
            <p>เก็บเสียงและจังหวะเงียบ<br />อย่างละเอียดบนอุปกรณ์</p>
            <div className="capture-wave" aria-label="ตัวอย่างคลื่นเสียง">
              <button type="button" aria-label="เล่นตัวอย่างเสียง"><AudioLines size={22} /></button>
              <div className="wave-bars" aria-hidden="true">
                {waveform.map((height, index) => <i key={index} style={{ height: `${height}%` }} />)}
              </div>
              <time>00:28:17</time>
            </div>
          </article>

          <article className="story-step story-understand">
            <div className="story-label"><b>02</b><span>เข้าใจ</span></div>
            <p>ถอดความเป็นข้อความ<br />พร้อมเวลาและผู้พูด</p>
            <div className="transcript-sheet">
              <small>00:02:31&nbsp;&nbsp; ผู้ดำเนินรายการ</small>
              <p>ช่วยอธิบายภาพรวมของสิ่งที่เรายังไม่ได้พูดถึงครับ</p>
              <small>00:02:38&nbsp;&nbsp; คุณวิทยา</small>
              <p>บางครั้งช่วงที่เงียบก็บอกบริบทได้มากกว่าคำตอบ</p>
            </div>
          </article>

          <article className="story-step story-connect">
            <div className="story-label"><b>03</b><span>เชื่อมโยง</span></div>
            <p>เชื่อมแนวคิดจากเนื้อหา<br />สู่ความรู้ที่อ้างอิงแหล่งจริง</p>
            <div className="graph" aria-label="ตัวอย่างความเชื่อมโยงของความรู้">
              <svg viewBox="0 0 100 100" aria-hidden="true">
                <line x1="50" y1="48" x2="24" y2="22" />
                <line x1="50" y1="48" x2="76" y2="22" />
                <line x1="50" y1="48" x2="18" y2="72" />
                <line x1="50" y1="48" x2="79" y2="73" />
                <line x1="24" y1="22" x2="76" y2="22" />
              </svg>
              {graphNodes.map((node) => (
                <span
                  key={node.label}
                  className={node.primary ? "graph-node graph-node-primary" : "graph-node"}
                  style={{ left: `${node.x}%`, top: `${node.y}%` }}
                >
                  {node.label}
                </span>
              ))}
            </div>
            <div className="provenance-line"><Fingerprint size={15} /> อ้างอิง 00:02:38 · ผู้พูด 02</div>
          </article>
        </div>
      </section>

      <section id="product" className="architecture-scene" data-scroll-scene>
        <div className="architecture-sticky">
          <div className="section-index" aria-hidden="true">03 / 04</div>
          <div className="architecture-copy">
            <h2>Local by default.<br />Connected by choice.</h2>
            <p>ข้อมูลหลักอยู่ใกล้คุณเสมอ<br />ส่วน Cloud เปิดใช้เมื่อคุณเลือก</p>
          </div>

          <div className="architecture-map">
            <div className="device-inputs">
              <span><AudioLines /> Audio</span>
              <span><FileText /> Notes</span>
              <span><FileText /> Transcript</span>
              <span><Network /> Graph</span>
              <span><Fingerprint /> Provenance</span>
            </div>

            <div className="local-device">
              <div className="device-bezel">
                <div className="device-layer device-layer-1" />
                <div className="device-layer device-layer-2" />
                <div className="device-core">
                  <Database aria-hidden="true" />
                  <strong>GenesisBlockDB</strong>
                  <span>Your data. Your rules.</span>
                </div>
              </div>
              <p>FUNG Desktop + GenesisBlockDB</p>
            </div>

            <div className="cloud-stack">
              <p>Optional cloud</p>
              <div><Cloud /><span><strong>Supabase</strong><small>Auth · Metadata · RLS</small></span></div>
              <div><span className="vercel-mark" /><span><strong>Vercel</strong><small>Web UI</small></span></div>
            </div>

            <div className="optional-connector" aria-hidden="true"><i /></div>
          </div>

          <div className="local-assurance">
            <LockKeyhole aria-hidden="true" size={20} />
            <span><strong>ใช้งานแบบ Local ได้โดยไม่ต้องมีบัญชี</strong><small>Cloud ไม่ใช่เงื่อนไขของการเก็บและค้นคืนความคิด</small></span>
          </div>
        </div>
      </section>

      <section id="privacy" className="closing-scene" data-scroll-scene>
        <div className="closing-ribbon" aria-hidden="true"><div className="memory-mark"><i /></div></div>
        <div className="closing-copy">
          <span className="closing-wordmark">FUNG</span>
          <h2>เก็บความคิดไว้ใกล้ตัว</h2>
          <p>เริ่มจากเสียงหนึ่งประโยค แล้วปล่อยให้ FUNG ช่วยรักษาบริบทที่เหลือ</p>
          <div className="closing-actions">
            <a className="landing-button landing-button-indigo" href="/app">เปิด FUNG <ArrowIcon /></a>
            <a className="landing-text-link" href="/app?surface=desktop">ดาวน์โหลด Desktop <ArrowIcon /></a>
          </div>
          <div className="privacy-note"><LockKeyhole size={17} /> ใช้งานแบบ Local ได้โดยไม่ต้องมีบัญชี</div>
        </div>
      </section>

      <footer className="landing-footer">
        <div className="footer-brand">
          <strong>FUNG</strong>
          <p>ระบบบันทึกเสียงที่เข้าใจบริบท<br />ทำงานในเครื่อง เคารพความเป็นส่วนตัว</p>
          <small>Built local-first</small>
        </div>
        <nav aria-label="เมนูส่วนท้าย">
          <a href="#product">Product <ArrowIcon /></a>
          <a href="#privacy">Privacy <ArrowIcon /></a>
          <a href="/app">Documentation <ArrowIcon /></a>
        </nav>
        <div className="footer-status">
          <small>SYSTEM STATUS</small>
          <p><i /> All Systems Local</p>
          <span>© FUNG. All rights reserved.</span>
        </div>
      </footer>
    </main>
  );
}

import { Mic, Upload } from "lucide-react";
import "./HomeScreen.css";

type LibraryItem = {
  id: string;
  title: string;
  subtitle: string;
  state: string;
};

interface HomeScreenProps {
  items: LibraryItem[];
  onStartRecording: () => void;
  onImport: () => void;
  onOpenItem: (id: string) => void;
}

export function HomeScreen({ items, onStartRecording, onImport, onOpenItem }: HomeScreenProps) {
  return (
    <div className="home-screen" data-tauri-drag-region>
      <div className="home-screen__brand">FUNG</div>

      <div className="home-screen__actions">
        <button type="button" className="home-screen__hero" onClick={onStartRecording}>
          <Mic size={18} />
          เริ่มบันทึกประชุม
        </button>
        <button type="button" className="home-screen__secondary" onClick={onImport}>
          <Upload size={16} />
          นำเข้าไฟล์เสียง
        </button>
      </div>

      <div className="home-screen__list-label">การประชุมล่าสุด</div>
      {items.length === 0 ? (
        <div className="home-screen__empty">
          <p>ยังไม่มีการประชุมที่บันทึกไว้</p>
          <p className="home-screen__empty-hint">
            กด “เริ่มบันทึกประชุม” เพื่อเริ่มบันทึกใหม่ หรือ “นำเข้าไฟล์เสียง” เพื่อถอดเสียงจากไฟล์ที่มีอยู่
          </p>
        </div>
      ) : (
        <div className="home-screen__list">
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              className="home-screen__row"
              onClick={() => onOpenItem(item.id)}
            >
              <span>{item.title}</span>
              <span className="home-screen__row-subtitle">
                {item.subtitle} · {item.state}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

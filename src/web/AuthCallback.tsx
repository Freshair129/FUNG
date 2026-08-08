import { useEffect, useState } from "react";
import { supabase } from "../lib/supabase";
import { LoadingScreen } from "./LoadingScreen";
import "./LoadingScreen.css";

export function AuthCallback() {
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    supabase.auth.getSession().then(({ data, error: sessionError }) => {
      if (sessionError || !data.session) {
        setError("เข้าสู่ระบบไม่สำเร็จ กรุณาลองใหม่");
        return;
      }
      window.location.href = "/app";
    });
  }, []);

  if (error) {
    return (
      <div className="auth-callback-error">
        <p>{error}</p>
        <a href="/">กลับหน้าแรก</a>
      </div>
    );
  }

  return <LoadingScreen message="กำลังเข้าสู่ระบบ..." />;
}

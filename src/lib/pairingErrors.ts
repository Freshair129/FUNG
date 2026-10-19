export interface PairingFailure {
  message: string;
  setupRequired: boolean;
}

/** Turn the native broker's stable public error codes into clear UI states. */
export function describePairingFailure(error: unknown, supabaseConfigured = true): PairingFailure {
  const code = typeof error === "string"
    ? error
    : error instanceof Error
      ? error.message
      : "";

  if (!supabaseConfigured || code.includes("auth_config_invalid")) {
    return {
      message: "ยังไม่ได้ตั้งค่า Supabase — เพิ่ม VITE_SUPABASE_URL และ VITE_SUPABASE_ANON_KEY เพื่อเปิดใช้บัญชีและการจับคู่อุปกรณ์",
      setupRequired: true,
    };
  }
  if (code.includes("auth_required")) {
    return { message: "กรุณาลงชื่อเข้าใช้ก่อนดูรายการหรือจับคู่อุปกรณ์", setupRequired: true };
  }
  if (code.includes("authorization_unavailable") || code.includes("auth_refresh_unavailable")) {
    return { message: "เชื่อมต่อบริการบัญชีไม่ได้ ตรวจสอบการเชื่อมต่อแล้วลองรีเฟรช", setupRequired: false };
  }
  return { message: "ทำรายการอุปกรณ์ไม่สำเร็จ ลองรีเฟรชอีกครั้ง", setupRequired: false };
}

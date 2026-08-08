# Auth + Login UI (Sub-project A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Google OAuth login via Supabase with a web dashboard shell, account settings panel, and login buttons on the landing page.

**Architecture:** Supabase Auth handles the OAuth handshake with Google. The frontend uses `@supabase/supabase-js` to initiate login, read session state, and query the `profiles` / `oauth_connections` tables. Routing is conditional rendering by pathname in `main.tsx` (existing FUNG pattern). An `AuthGuard` wrapper redirects unauthenticated users from the web dashboard back to the landing page.

**Tech Stack:** React 18, TypeScript, @supabase/supabase-js v2, Vite, Supabase (hosted), Vercel

## Global Constraints

- UI labels in Thai; code identifiers in English
- Named exports only (no default exports) — matches `App`, `LandingPage`, `MobileApp`, `ExternalAccountPanel`, `TtsProviderPanel`
- CSS pattern: hardcoded light-theme values + `.theme-dark .prefix-*` overrides (no CSS custom properties for colors)
- No React Router — conditional render by `window.location.pathname` in `main.tsx`
- Supabase project ref: `nqnrvqnijzovkrhxslfp`
- Anon key only in frontend — no service role key
- `.env` is already gitignored (lines 19-21 of `.gitignore`)
- `VITE_` prefix for env vars (Vite convention, already configured in `vite.config.ts` line 14)
- Desktop/mobile surfaces (`/app?surface=desktop`, `/app?surface=mobile`) must NOT be gated by auth
- Existing `ExternalAccountPanel` already reads `VITE_SUPABASE_URL` (line 10 of `ExternalAccountPanel.tsx`) — the env var name is established

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `src/lib/supabase.ts` | Supabase client singleton |
| `src/web/AuthCallback.tsx` | OAuth callback handler (`/auth/callback`) |
| `src/web/AuthGuard.tsx` | Session check + redirect wrapper |
| `src/web/LoadingScreen.tsx` | Shared loading component (logo + spinner) |
| `src/web/LoadingScreen.css` | Loading screen styles |
| `src/web/Dashboard.tsx` | Post-login web dashboard shell |
| `src/web/Dashboard.css` | Dashboard styles |
| `src/web/AccountSettings.tsx` | Account settings slide-over panel |
| `src/web/AccountSettings.css` | Account settings styles |
| `.env.example` | Environment variable template |
| `supabase/migrations/20260808000000_profile_trigger.sql` | Profile + OAuth connection auto-provisioning triggers |

### Modified files

| File | Change |
|------|--------|
| `src/main.tsx` | Add route branching for `/auth/callback` and web `/app` |
| `src/landing/LandingPage.tsx` | Login/avatar in header + hero + closing CTA |
| `src/landing/landing.css` | Login button, avatar button, avatar dropdown styles |
| `package.json` | Add `@supabase/supabase-js` |

---

### Task 1: Supabase Client + Environment Setup

**Files:**
- Create: `src/lib/supabase.ts`
- Create: `.env.example`
- Modify: `package.json`

**Interfaces:**
- Consumes: nothing
- Produces: `supabase` — a `SupabaseClient` instance exported from `src/lib/supabase.ts`. All later tasks import this.

- [ ] **Step 1: Install @supabase/supabase-js**

```bash
npm install @supabase/supabase-js
```

Expected: package.json updated with `"@supabase/supabase-js": "^2.x"` in dependencies.

- [ ] **Step 2: Create .env.example**

Create `.env.example`:

```
# Supabase project credentials (public — safe to embed in frontend bundle)
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_ANON_KEY=your-anon-key
```

- [ ] **Step 3: Create .env with real values**

Create `.env` (gitignored — will NOT be committed):

```
VITE_SUPABASE_URL=https://nqnrvqnijzovkrhxslfp.supabase.co
VITE_SUPABASE_ANON_KEY=<actual-anon-key>
```

To get the anon key: Supabase Dashboard → Settings → API → `anon` `public` key.

**Note:** If the actual anon key is not available during implementation, use a placeholder. The tests in this task don't hit Supabase — they only verify the module compiles and exports correctly.

- [ ] **Step 4: Create src/lib/supabase.ts**

```typescript
import { createClient } from "@supabase/supabase-js";

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL as string;
const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY as string;

if (!supabaseUrl || !supabaseAnonKey) {
  console.warn(
    "Supabase credentials missing. Set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY in .env"
  );
}

export const supabase = createClient(supabaseUrl ?? "", supabaseAnonKey ?? "");
```

- [ ] **Step 5: Verify TypeScript compiles**

```bash
npx tsc --noEmit
```

Expected: No new errors (pre-existing `PitchingAssist.tsx` errors are unrelated).

- [ ] **Step 6: Verify Vite build**

```bash
npx vite build
```

Expected: Build succeeds. `supabase.ts` is tree-shaken if not imported by anything yet, but the module is syntactically valid.

- [ ] **Step 7: Commit**

```bash
git add src/lib/supabase.ts .env.example package.json package-lock.json
git commit -m "feat(auth): add Supabase client and env template"
```

Do NOT commit `.env` — it is gitignored.

---

### Task 2: Supabase Migration — Profile Trigger

**Files:**
- Create: `supabase/migrations/20260808000000_profile_trigger.sql`

**Interfaces:**
- Consumes: existing `profiles` table (columns: `id uuid PK`, `display_name text`) and `oauth_connections` table (columns: `user_id uuid`, `provider text`, `status text`, `approved_scopes text[]`, unique constraint on `(user_id, provider)`) from migration `20260722000000_auth_control_plane.sql`
- Produces: two database triggers that fire on `auth.users` INSERT — downstream code can rely on `profiles` and `oauth_connections` rows existing for any authenticated user

- [ ] **Step 1: Create the migration file**

Create `supabase/migrations/20260808000000_profile_trigger.sql`:

```sql
-- Auto-create profile row when a new user signs up via Supabase Auth.
-- The profiles table and its RLS policies already exist in
-- 20260722000000_auth_control_plane.sql — this migration only adds the
-- trigger that populates it on first login.

CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger AS $$
BEGIN
  INSERT INTO public.profiles (id, display_name)
  VALUES (
    NEW.id,
    COALESCE(
      NEW.raw_user_meta_data->>'full_name',
      NEW.raw_user_meta_data->>'name',
      'User'
    )
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE TRIGGER on_auth_user_created
  AFTER INSERT ON auth.users
  FOR EACH ROW EXECUTE FUNCTION public.handle_new_user();

-- Auto-create an oauth_connections row for Google sign-ups so the
-- "connected accounts" UI reflects the login provider immediately.

CREATE OR REPLACE FUNCTION public.handle_google_oauth_connection()
RETURNS trigger AS $$
BEGIN
  IF NEW.raw_app_meta_data->>'provider' = 'google' THEN
    INSERT INTO public.oauth_connections (user_id, provider, status, approved_scopes)
    VALUES (NEW.id, 'google', 'active', ARRAY['openid', 'email', 'profile'])
    ON CONFLICT (user_id, provider) DO UPDATE
    SET status = 'active', updated_at = now();
  END IF;
  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE TRIGGER on_auth_user_google_connection
  AFTER INSERT ON auth.users
  FOR EACH ROW EXECUTE FUNCTION public.handle_google_oauth_connection();
```

- [ ] **Step 2: Verify SQL syntax**

Read the file back and confirm:
- Both functions use `SECURITY DEFINER` (required to bypass RLS when writing to `profiles` and `oauth_connections`)
- `ON CONFLICT (user_id, provider)` matches the existing unique constraint `oauth_connections_unique_provider_per_user` in `20260722000000_auth_control_plane.sql` line 51
- Column `updated_at` exists on `oauth_connections` — verify: it does NOT exist in the current schema. The `oauth_connections` table has `connected_at`, `revoked_at`, `last_authorized_at` but no `updated_at`.

Fix the ON CONFLICT clause:

```sql
    ON CONFLICT (user_id, provider) DO UPDATE
    SET status = 'active', last_authorized_at = now();
```

- [ ] **Step 3: Commit**

```bash
git add supabase/migrations/20260808000000_profile_trigger.sql
git commit -m "feat(auth): add profile and OAuth connection auto-provisioning triggers"
```

**Note:** This migration is applied to the remote Supabase project via `supabase db push` or the Supabase Dashboard SQL editor. It does not run locally — there is no local Supabase dev environment configured for this project.

---

### Task 3: LoadingScreen + AuthCallback + AuthGuard

**Files:**
- Create: `src/web/LoadingScreen.tsx`
- Create: `src/web/LoadingScreen.css`
- Create: `src/web/AuthCallback.tsx`
- Create: `src/web/AuthGuard.tsx`

**Interfaces:**
- Consumes: `supabase` from `src/lib/supabase.ts` (Task 1), `FungLogo` from `src/components/FungLogo.tsx`
- Produces:
  - `LoadingScreen({ message?: string }): JSX.Element` — shared loading UI
  - `AuthCallback(): JSX.Element` — renders at `/auth/callback`, reads hash tokens, redirects to `/app`
  - `AuthGuard({ children }: { children: React.ReactNode }): JSX.Element` — wraps Dashboard, redirects to `/` if unauthenticated

- [ ] **Step 1: Create src/web/LoadingScreen.css**

```css
.loading-screen {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  gap: 24px;
  background: #f5f2eb;
}

.loading-spinner {
  width: 28px;
  height: 28px;
  border: 3px solid rgba(111, 137, 126, 0.25);
  border-top-color: #6f897e;
  border-radius: 50%;
  animation: loading-spin 0.8s linear infinite;
}

.loading-message {
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 14px;
  color: #5f6268;
  margin: 0;
}

@keyframes loading-spin {
  to { transform: rotate(360deg); }
}

.theme-dark .loading-screen {
  background: #171918;
}

.theme-dark .loading-spinner {
  border-color: rgba(163, 179, 170, 0.25);
  border-top-color: #a3b3aa;
}

.theme-dark .loading-message {
  color: #9b9386;
}

/* Auth callback error state */
.auth-callback-error {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100vh;
  gap: 16px;
  background: #f5f2eb;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
}

.auth-callback-error p {
  font-size: 16px;
  color: #c0392b;
  margin: 0;
}

.auth-callback-error a {
  font-size: 14px;
  color: #3d4f82;
  text-decoration: none;
}

.auth-callback-error a:hover {
  text-decoration: underline;
}

.theme-dark .auth-callback-error {
  background: #171918;
}

.theme-dark .auth-callback-error p {
  color: #e74c3c;
}

.theme-dark .auth-callback-error a {
  color: #a8b6e6;
}
```

- [ ] **Step 2: Create src/web/LoadingScreen.tsx**

```tsx
import { FungLogo } from "../components/FungLogo";
import "./LoadingScreen.css";

type LoadingScreenProps = {
  message?: string;
};

export function LoadingScreen({ message }: LoadingScreenProps) {
  return (
    <div className="loading-screen">
      <FungLogo size={48} />
      <div className="loading-spinner" />
      {message && <p className="loading-message">{message}</p>}
    </div>
  );
}
```

- [ ] **Step 3: Create src/web/AuthCallback.tsx**

```tsx
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
```

- [ ] **Step 4: Create src/web/AuthGuard.tsx**

```tsx
import { useEffect, useState } from "react";
import { supabase } from "../lib/supabase";
import { LoadingScreen } from "./LoadingScreen";

type AuthGuardProps = {
  children: React.ReactNode;
};

export function AuthGuard({ children }: AuthGuardProps) {
  const [state, setState] = useState<"loading" | "authenticated" | "unauthenticated">("loading");

  useEffect(() => {
    supabase.auth.getSession().then(({ data }) => {
      setState(data.session ? "authenticated" : "unauthenticated");
    });

    const {
      data: { subscription },
    } = supabase.auth.onAuthStateChange((_event, session) => {
      setState(session ? "authenticated" : "unauthenticated");
    });

    return () => subscription.unsubscribe();
  }, []);

  if (state === "loading") {
    return <LoadingScreen />;
  }

  if (state === "unauthenticated") {
    window.location.href = "/";
    return null;
  }

  return <>{children}</>;
}
```

- [ ] **Step 5: Verify TypeScript compiles**

```bash
npx tsc --noEmit
```

Expected: No new errors.

- [ ] **Step 6: Commit**

```bash
git add src/web/LoadingScreen.tsx src/web/LoadingScreen.css src/web/AuthCallback.tsx src/web/AuthGuard.tsx
git commit -m "feat(auth): add LoadingScreen, AuthCallback, and AuthGuard"
```

---

### Task 4: Dashboard + Account Settings

**Files:**
- Create: `src/web/Dashboard.tsx`
- Create: `src/web/Dashboard.css`
- Create: `src/web/AccountSettings.tsx`
- Create: `src/web/AccountSettings.css`

**Interfaces:**
- Consumes: `supabase` from `src/lib/supabase.ts` (Task 1), `FungLogo` from `src/components/FungLogo.tsx`
- Produces: `Dashboard(): JSX.Element` — the post-login web shell, used by `main.tsx` routing in Task 5

- [ ] **Step 1: Create src/web/AccountSettings.css**

```css
.account-settings-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.35);
  z-index: 900;
  display: flex;
  justify-content: flex-end;
}

.account-settings-panel {
  width: 420px;
  max-width: 100vw;
  height: 100vh;
  background: #fffdf8;
  overflow-y: auto;
  padding: 32px 28px;
  box-shadow: -4px 0 24px rgba(0, 0, 0, 0.08);
  animation: account-settings-slide-in 200ms ease-out;
}

@keyframes account-settings-slide-in {
  from { transform: translateX(100%); }
  to { transform: translateX(0); }
}

.account-settings-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 28px;
}

.account-settings-header h2 {
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 18px;
  font-weight: 600;
  color: #191a1d;
  margin: 0;
  display: flex;
  align-items: center;
  gap: 10px;
}

.account-settings-back {
  background: none;
  border: none;
  cursor: pointer;
  color: #5f6268;
  padding: 4px;
  display: flex;
  align-items: center;
}

.account-settings-section {
  margin-bottom: 24px;
}

.account-settings-section-title {
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 14px;
  font-weight: 600;
  color: #191a1d;
  margin: 0 0 12px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.account-settings-card {
  background: #f7f2ea;
  border-radius: 10px;
  padding: 16px;
}

.account-settings-field {
  margin-bottom: 12px;
}

.account-settings-field:last-child {
  margin-bottom: 0;
}

.account-settings-label {
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 12px;
  color: #5f6268;
  margin-bottom: 4px;
  display: block;
}

.account-settings-input {
  width: 100%;
  padding: 8px 12px;
  border: 1px solid rgba(32, 35, 40, 0.12);
  border-radius: 6px;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 14px;
  color: #191a1d;
  background: #fffdf8;
  box-sizing: border-box;
}

.account-settings-input:focus {
  outline: none;
  border-color: #3d4f82;
}

.account-settings-input[readonly] {
  background: #f0ece3;
  color: #5f6268;
  cursor: default;
}

.account-settings-save-btn {
  background: #3d4f82;
  color: #fff;
  border: none;
  border-radius: 6px;
  padding: 8px 20px;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  margin-top: 8px;
}

.account-settings-save-btn:hover {
  background: #2e3d66;
}

.account-settings-save-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.account-settings-connected {
  display: flex;
  align-items: center;
  gap: 8px;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 13px;
  color: #2e7d32;
}

.account-settings-placeholder {
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 13px;
  color: #5f6268;
  padding: 12px 0;
}

.account-settings-message {
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 13px;
  margin-top: 8px;
}

.account-settings-message.success {
  color: #2e7d32;
}

.account-settings-message.error {
  color: #c0392b;
}

/* Dark theme overrides */
.theme-dark .account-settings-panel {
  background: linear-gradient(165deg, #1e2120 0%, #171918 100%);
  box-shadow: -4px 0 24px rgba(0, 0, 0, 0.3);
}

.theme-dark .account-settings-header h2 {
  color: #f3efe7;
}

.theme-dark .account-settings-back {
  color: #9b9386;
}

.theme-dark .account-settings-section-title {
  color: #f3efe7;
}

.theme-dark .account-settings-card {
  background: rgba(255, 255, 255, 0.04);
}

.theme-dark .account-settings-label {
  color: #9b9386;
}

.theme-dark .account-settings-input {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.08);
  color: #f3efe7;
}

.theme-dark .account-settings-input[readonly] {
  background: rgba(255, 255, 255, 0.03);
  color: #9b9386;
}

.theme-dark .account-settings-save-btn {
  background: #a8b6e6;
  color: #171918;
}

.theme-dark .account-settings-save-btn:hover {
  background: #8fa0d8;
}

.theme-dark .account-settings-connected {
  color: #66bb6a;
}

.theme-dark .account-settings-placeholder {
  color: #9b9386;
}
```

- [ ] **Step 2: Create src/web/AccountSettings.tsx**

```tsx
import { useEffect, useState } from "react";
import { ArrowLeft, CheckCircle2, Cloud, Link2, Monitor, User, X } from "lucide-react";
import { supabase } from "../lib/supabase";
import "./AccountSettings.css";

type AccountSettingsProps = {
  onClose: () => void;
};

type OAuthConnection = {
  id: string;
  provider: string;
  status: string;
};

export function AccountSettings({ onClose }: AccountSettingsProps) {
  const [email, setEmail] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [savedName, setSavedName] = useState("");
  const [connections, setConnections] = useState<OAuthConnection[]>([]);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ type: "success" | "error"; text: string } | null>(null);

  useEffect(() => {
    const load = async () => {
      const {
        data: { user },
      } = await supabase.auth.getUser();
      if (!user) return;

      setEmail(user.email ?? "");

      const { data: profile } = await supabase
        .from("profiles")
        .select("display_name")
        .eq("id", user.id)
        .single();

      if (profile) {
        setDisplayName(profile.display_name ?? "");
        setSavedName(profile.display_name ?? "");
      }

      const { data: oauthConnections } = await supabase
        .from("oauth_connections")
        .select("id, provider, status")
        .eq("user_id", user.id);

      if (oauthConnections) {
        setConnections(oauthConnections);
      }
    };

    void load();
  }, []);

  const handleSave = async () => {
    const trimmed = displayName.trim();
    if (!trimmed || trimmed === savedName) return;

    setSaving(true);
    setMessage(null);

    try {
      const {
        data: { user },
      } = await supabase.auth.getUser();
      if (!user) throw new Error("ไม่พบ session");

      const { error } = await supabase
        .from("profiles")
        .update({ display_name: trimmed })
        .eq("id", user.id);

      if (error) throw error;

      setSavedName(trimmed);
      setMessage({ type: "success", text: "บันทึกแล้ว" });
    } catch (err) {
      setMessage({
        type: "error",
        text: err instanceof Error ? err.message : "บันทึกไม่สำเร็จ",
      });
    } finally {
      setSaving(false);
    }
  };

  const nameChanged = displayName.trim() !== savedName && displayName.trim().length > 0;

  return (
    <div className="account-settings-overlay" role="presentation" onMouseDown={onClose}>
      <section
        className="account-settings-panel"
        aria-label="ตั้งค่าบัญชี"
        aria-modal="true"
        role="dialog"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <header className="account-settings-header">
          <h2>
            <button
              className="account-settings-back"
              type="button"
              onClick={onClose}
              aria-label="ปิด"
            >
              <ArrowLeft size={18} />
            </button>
            ตั้งค่าบัญชี
          </h2>
          <button
            className="account-settings-back"
            type="button"
            onClick={onClose}
            aria-label="ปิด"
          >
            <X size={18} />
          </button>
        </header>

        {/* Profile section */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <User size={16} /> โปรไฟล์
          </h3>
          <div className="account-settings-card">
            <div className="account-settings-field">
              <label className="account-settings-label">ชื่อแสดง</label>
              <input
                className="account-settings-input"
                type="text"
                value={displayName}
                onChange={(e) => {
                  setDisplayName(e.target.value);
                  setMessage(null);
                }}
                maxLength={120}
              />
            </div>
            <div className="account-settings-field">
              <label className="account-settings-label">อีเมล</label>
              <input
                className="account-settings-input"
                type="email"
                value={email}
                readOnly
              />
            </div>
            <button
              className="account-settings-save-btn"
              type="button"
              onClick={() => void handleSave()}
              disabled={saving || !nameChanged}
            >
              {saving ? "กำลังบันทึก..." : "บันทึก"}
            </button>
            {message && (
              <p className={`account-settings-message ${message.type}`}>{message.text}</p>
            )}
          </div>
        </div>

        {/* Connected accounts section */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <Link2 size={16} /> บัญชีที่เชื่อมต่อ
          </h3>
          <div className="account-settings-card">
            {connections.filter((c) => c.status === "active").length > 0 ? (
              connections
                .filter((c) => c.status === "active")
                .map((c) => (
                  <div key={c.id} className="account-settings-connected">
                    <CheckCircle2 size={16} />
                    {c.provider === "google" ? "Google" : c.provider} — เชื่อมต่อแล้ว
                  </div>
                ))
            ) : (
              <p className="account-settings-placeholder">ไม่มีบัญชีที่เชื่อมต่อ</p>
            )}
          </div>
        </div>

        {/* Cloud Storage placeholder */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <Cloud size={16} /> Cloud Storage
          </h3>
          <div className="account-settings-card">
            <p className="account-settings-placeholder">ยังไม่พร้อมใช้งาน</p>
          </div>
        </div>

        {/* Paired devices placeholder */}
        <div className="account-settings-section">
          <h3 className="account-settings-section-title">
            <Monitor size={16} /> อุปกรณ์ที่จับคู่
          </h3>
          <div className="account-settings-card">
            <p className="account-settings-placeholder">ยังไม่พร้อมใช้งาน</p>
          </div>
        </div>
      </section>
    </div>
  );
}
```

- [ ] **Step 3: Create src/web/Dashboard.css**

```css
.dashboard {
  min-height: 100vh;
  background: #f5f2eb;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
}

/* Top bar */
.dashboard-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 28px;
  border-bottom: 1px solid rgba(32, 35, 40, 0.08);
}

.dashboard-topbar-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.dashboard-topbar-left .web-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: #5f6268;
  background: rgba(111, 137, 126, 0.1);
  padding: 2px 8px;
  border-radius: 10px;
}

.dashboard-topbar-right {
  display: flex;
  align-items: center;
  gap: 12px;
}

/* Avatar button */
.dashboard-avatar-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  background: none;
  border: 1px solid rgba(32, 35, 40, 0.12);
  border-radius: 20px;
  padding: 4px 12px 4px 4px;
  cursor: pointer;
  position: relative;
}

.dashboard-avatar-btn img {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  object-fit: cover;
}

.dashboard-avatar-btn .avatar-fallback {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  background: #6f897e;
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 13px;
  font-weight: 600;
}

/* Avatar dropdown */
.dashboard-avatar-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  background: #fffdf8;
  border: 1px solid rgba(32, 35, 40, 0.1);
  border-radius: 10px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.08);
  padding: 6px 0;
  min-width: 180px;
  z-index: 800;
}

.dashboard-avatar-dropdown button {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 10px 16px;
  background: none;
  border: none;
  cursor: pointer;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 13px;
  color: #191a1d;
  text-align: left;
}

.dashboard-avatar-dropdown button:hover {
  background: rgba(111, 137, 126, 0.08);
}

/* Main content */
.dashboard-main {
  max-width: 680px;
  margin: 0 auto;
  padding: 60px 28px;
  text-align: center;
}

.dashboard-welcome h1 {
  font-size: 24px;
  font-weight: 600;
  color: #191a1d;
  margin: 0 0 8px;
}

.dashboard-welcome p {
  font-size: 15px;
  color: #5f6268;
  margin: 0 0 40px;
}

.dashboard-tiles {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.dashboard-tile {
  background: #fffdf8;
  border: 1px solid rgba(32, 35, 40, 0.08);
  border-radius: 12px;
  padding: 24px;
  text-align: left;
  cursor: default;
  opacity: 0.6;
}

.dashboard-tile-icon {
  font-size: 28px;
  margin-bottom: 12px;
}

.dashboard-tile h3 {
  font-size: 15px;
  font-weight: 600;
  color: #191a1d;
  margin: 0 0 4px;
}

.dashboard-tile p {
  font-size: 12px;
  color: #5f6268;
  margin: 0;
}

/* Dark theme */
.theme-dark .dashboard {
  background: #171918;
}

.theme-dark .dashboard-topbar {
  border-bottom-color: rgba(255, 255, 255, 0.06);
}

.theme-dark .dashboard-topbar-left .web-badge {
  color: #9b9386;
  background: rgba(163, 179, 170, 0.1);
}

.theme-dark .dashboard-avatar-btn {
  border-color: rgba(255, 255, 255, 0.1);
}

.theme-dark .dashboard-avatar-dropdown {
  background: #1e2120;
  border-color: rgba(255, 255, 255, 0.08);
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.3);
}

.theme-dark .dashboard-avatar-dropdown button {
  color: #f3efe7;
}

.theme-dark .dashboard-avatar-dropdown button:hover {
  background: rgba(163, 179, 170, 0.08);
}

.theme-dark .dashboard-welcome h1 {
  color: #f3efe7;
}

.theme-dark .dashboard-welcome p {
  color: #9b9386;
}

.theme-dark .dashboard-tile {
  background: rgba(255, 255, 255, 0.03);
  border-color: rgba(255, 255, 255, 0.06);
}

.theme-dark .dashboard-tile h3 {
  color: #f3efe7;
}

.theme-dark .dashboard-tile p {
  color: #9b9386;
}
```

- [ ] **Step 4: Create src/web/Dashboard.tsx**

```tsx
import { useEffect, useState } from "react";
import { ChevronDown, Cloud, LogOut, Mic, FolderOpen, Settings } from "lucide-react";
import { FungLogo } from "../components/FungLogo";
import { supabase } from "../lib/supabase";
import { AccountSettings } from "./AccountSettings";
import "./Dashboard.css";

export function Dashboard() {
  const [displayName, setDisplayName] = useState("");
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [dropdownOpen, setDropdownOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);

  useEffect(() => {
    const load = async () => {
      const {
        data: { user },
      } = await supabase.auth.getUser();
      if (!user) return;

      setAvatarUrl(user.user_metadata?.avatar_url ?? null);

      const { data: profile } = await supabase
        .from("profiles")
        .select("display_name")
        .eq("id", user.id)
        .single();

      if (profile) {
        setDisplayName(profile.display_name ?? "User");
      }
    };

    void load();
  }, []);

  const handleSignOut = async () => {
    await supabase.auth.signOut();
    window.location.href = "/";
  };

  // Close dropdown when clicking outside
  useEffect(() => {
    if (!dropdownOpen) return;
    const close = () => setDropdownOpen(false);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  }, [dropdownOpen]);

  return (
    <div className="dashboard">
      <header className="dashboard-topbar">
        <div className="dashboard-topbar-left">
          <FungLogo size={28} />
          <span className="web-badge">
            <Cloud size={12} /> Web
          </span>
        </div>

        <div className="dashboard-topbar-right">
          <button
            className="dashboard-avatar-btn"
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              setDropdownOpen((prev) => !prev);
            }}
          >
            {avatarUrl ? (
              <img src={avatarUrl} alt="" referrerPolicy="no-referrer" />
            ) : (
              <span className="avatar-fallback">
                {displayName.charAt(0).toUpperCase() || "U"}
              </span>
            )}
            <ChevronDown size={14} />

            {dropdownOpen && (
              <div className="dashboard-avatar-dropdown">
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    setDropdownOpen(false);
                    setSettingsOpen(true);
                  }}
                >
                  <Settings size={15} /> ตั้งค่าบัญชี
                </button>
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    void handleSignOut();
                  }}
                >
                  <LogOut size={15} /> ออกจากระบบ
                </button>
              </div>
            )}
          </button>
        </div>
      </header>

      <main className="dashboard-main">
        <div className="dashboard-welcome">
          <h1>ยินดีต้อนรับสู่ FUNG Web</h1>
          <p>{displayName}</p>
        </div>

        <div className="dashboard-tiles">
          <div className="dashboard-tile">
            <div className="dashboard-tile-icon">🎙️</div>
            <h3>เริ่มบันทึก</h3>
            <p>เร็วๆ นี้</p>
          </div>
          <div className="dashboard-tile">
            <div className="dashboard-tile-icon">📁</div>
            <h3>ไฟล์ล่าสุด</h3>
            <p>เร็วๆ นี้</p>
          </div>
        </div>
      </main>

      {settingsOpen && <AccountSettings onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}
```

- [ ] **Step 5: Verify TypeScript compiles**

```bash
npx tsc --noEmit
```

Expected: No new errors.

- [ ] **Step 6: Commit**

```bash
git add src/web/Dashboard.tsx src/web/Dashboard.css src/web/AccountSettings.tsx src/web/AccountSettings.css
git commit -m "feat(auth): add web Dashboard shell and AccountSettings panel"
```

---

### Task 5: Routing + Landing Page Login

**Files:**
- Modify: `src/main.tsx`
- Modify: `src/landing/LandingPage.tsx`
- Modify: `src/landing/landing.css`

**Interfaces:**
- Consumes: `AuthCallback` from `src/web/AuthCallback.tsx` (Task 3), `AuthGuard` from `src/web/AuthGuard.tsx` (Task 3), `Dashboard` from `src/web/Dashboard.tsx` (Task 4), `supabase` from `src/lib/supabase.ts` (Task 1)
- Produces: complete auth flow end-to-end — landing → login → callback → dashboard

- [ ] **Step 1: Modify src/main.tsx to add route branching**

Replace the entire content of `src/main.tsx` with:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { LandingPage } from "./landing/LandingPage";
import { MobileApp } from "./mobile/MobileApp";
import { AuthCallback } from "./web/AuthCallback";
import { AuthGuard } from "./web/AuthGuard";
import { Dashboard } from "./web/Dashboard";
import "./styles.css";

const params = new URLSearchParams(window.location.search);
const path = window.location.pathname;
const isTauriRuntime = "__TAURI_INTERNALS__" in window;
const surface = params.get("surface");

function RootRouter() {
  // Tauri desktop runtime — always show the desktop app, no auth
  if (isTauriRuntime) {
    return <App />;
  }

  // OAuth callback — process tokens
  if (path === "/auth/callback") {
    return <AuthCallback />;
  }

  // /app with explicit surface — desktop/mobile app, no auth gate
  if (path === "/app" && (surface === "desktop" || surface === "mobile")) {
    const mobileViewport = window.matchMedia(
      "(pointer: coarse) and (max-width: 760px), (pointer: coarse) and (orientation: landscape) and (max-height: 760px)",
    ).matches;
    const ProductApp = surface === "mobile" || (!surface && mobileViewport) ? MobileApp : App;
    return <ProductApp />;
  }

  // /app without surface — web dashboard, requires auth
  if (path === "/app") {
    return (
      <AuthGuard>
        <Dashboard />
      </AuthGuard>
    );
  }

  // Landing page (default)
  return <LandingPage />;
}

document.body.dataset.surface =
  path === "/" || (!isTauriRuntime && path !== "/app" && path !== "/auth/callback")
    ? "landing"
    : "app";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <RootRouter />
  </React.StrictMode>,
);
```

- [ ] **Step 2: Add login styles to src/landing/landing.css**

Append the following at the end of `landing.css`, before the existing media query block (`@media (max-width: 820px)`):

```css
/* Auth login button in header */
.landing-login-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  padding: 0 18px;
  min-height: 44px;
  background: #fffdf8;
  color: #191a1d;
  border: 1px solid rgba(32, 35, 40, 0.15);
  border-radius: 10px;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  text-decoration: none;
  transition: transform 220ms ease, background 220ms ease;
}

.landing-login-btn:hover {
  background: #f7f2ea;
  transform: translateY(-2px);
}

.landing-login-btn svg {
  flex-shrink: 0;
}

/* Hero login CTA */
.hero-login-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: #fffdf8;
  color: #191a1d;
  border: 1px solid rgba(32, 35, 40, 0.15);
  border-radius: 10px;
  padding: 12px 24px;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 15px;
  font-weight: 500;
  cursor: pointer;
  text-decoration: none;
  transition: background 220ms ease, transform 220ms ease;
}

.hero-login-btn:hover {
  background: #f7f2ea;
  transform: translateY(-1px);
}

/* Avatar in header when logged in */
.landing-avatar-btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  background: none;
  border: 1px solid rgba(32, 35, 40, 0.15);
  border-radius: 20px;
  padding: 3px 10px 3px 3px;
  cursor: pointer;
  position: relative;
}

.landing-avatar-btn img {
  width: 28px;
  height: 28px;
  border-radius: 50%;
  object-fit: cover;
}

.landing-avatar-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  background: #fffdf8;
  border: 1px solid rgba(32, 35, 40, 0.1);
  border-radius: 10px;
  box-shadow: 0 6px 20px rgba(0, 0, 0, 0.08);
  padding: 6px 0;
  min-width: 170px;
  z-index: 800;
}

.landing-avatar-dropdown button {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 10px 16px;
  background: none;
  border: none;
  cursor: pointer;
  font-family: "IBM Plex Sans Thai", "DM Sans", system-ui, sans-serif;
  font-size: 13px;
  color: #191a1d;
  text-align: left;
}

.landing-avatar-dropdown button:hover {
  background: rgba(111, 137, 126, 0.08);
}

/* Google G icon inline SVG */
.google-icon {
  width: 18px;
  height: 18px;
  flex-shrink: 0;
}
```

- [ ] **Step 3: Modify src/landing/LandingPage.tsx to add login/avatar**

Add the following imports at the top of the file, after the existing imports:

```tsx
import { useEffect, useState } from "react";
// (useEffect is already imported — just ensure useState is too)
import { ChevronDown, LogIn, LogOut, Settings } from "lucide-react";
import { supabase } from "../lib/supabase";
import type { User } from "@supabase/supabase-js";
```

Inside `export function LandingPage()`, after the existing `useScrollNarrative()` and `useEffect` for title, add:

```tsx
const [user, setUser] = useState<User | null>(null);
const [avatarDropdownOpen, setAvatarDropdownOpen] = useState(false);

useEffect(() => {
  supabase.auth.getSession().then(({ data }) => {
    setUser(data.session?.user ?? null);
  });
}, []);

useEffect(() => {
  if (!avatarDropdownOpen) return;
  const close = () => setAvatarDropdownOpen(false);
  document.addEventListener("click", close);
  return () => document.removeEventListener("click", close);
}, [avatarDropdownOpen]);

const handleLogin = () => {
  void supabase.auth.signInWithOAuth({
    provider: "google",
    options: { redirectTo: `${window.location.origin}/auth/callback` },
  });
};

const handleSignOut = () => {
  void supabase.auth.signOut().then(() => {
    setUser(null);
  });
};
```

In the header `<div>` that contains the CTAs (around line 96-103), add the login/avatar button before "เปิด FUNG":

```tsx
<div style={{ display: "flex", gap: "10px", alignItems: "center" }}>
  <a className="landing-header-cta" href="/fung-android-debug.apk" download style={{ background: "rgba(111, 137, 126, 0.12)", color: "#3d4f82", border: "1px solid rgba(61, 79, 130, 0.2)" }}>
    <Download size={15} style={{ marginRight: 6 }} /> โหลด APK
  </a>
  {user ? (
    <button
      className="landing-avatar-btn"
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        setAvatarDropdownOpen((prev) => !prev);
      }}
    >
      {user.user_metadata?.avatar_url ? (
        <img src={user.user_metadata.avatar_url} alt="" referrerPolicy="no-referrer" />
      ) : (
        <LogIn size={16} />
      )}
      <ChevronDown size={14} />
      {avatarDropdownOpen && (
        <div className="landing-avatar-dropdown">
          <button type="button" onClick={() => { window.location.href = "/app"; }}>
            <Settings size={15} /> ตั้งค่าบัญชี
          </button>
          <button type="button" onClick={(e) => { e.stopPropagation(); handleSignOut(); }}>
            <LogOut size={15} /> ออกจากระบบ
          </button>
        </div>
      )}
    </button>
  ) : (
    <button className="landing-login-btn" type="button" onClick={handleLogin}>
      <LogIn size={15} /> เข้าสู่ระบบ
    </button>
  )}
  <a className="landing-header-cta" href="/app">
    เปิด FUNG <ArrowIcon />
  </a>
</div>
```

In the hero actions `<div>` (around line 121-131), add login button after the primary CTA:

```tsx
<div className="hero-actions">
  <a className="landing-button landing-button-primary" href="/app">
    เปิด FUNG <ArrowIcon />
  </a>
  {!user && (
    <button className="hero-login-btn" type="button" onClick={handleLogin}>
      <svg className="google-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92a5.06 5.06 0 0 1-2.2 3.32v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.1z" fill="#4285F4"/>
        <path d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z" fill="#34A853"/>
        <path d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z" fill="#FBBC05"/>
        <path d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z" fill="#EA4335"/>
      </svg>
      เข้าสู่ระบบด้วย Google
    </button>
  )}
  <a className="landing-button landing-button-secondary" href="/fung-android-debug.apk" download style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
    <Download size={18} /> ดาวน์โหลด Android APK
  </a>
  <a className="landing-button landing-button-secondary" href="#how-it-works">
    ดูวิธีทำงาน <ArrowDown aria-hidden="true" size={17} strokeWidth={1.6} />
  </a>
</div>
```

In the closing actions `<div>` (around line 436-441), add login link:

```tsx
<div className="closing-actions">
  <a className="landing-button landing-button-indigo" href="/app">
    เปิด FUNG <ArrowIcon />
  </a>
  {!user && (
    <button
      className="landing-text-link"
      type="button"
      onClick={handleLogin}
      style={{ background: "none", border: "none", cursor: "pointer", fontFamily: "inherit", fontSize: "inherit", color: "inherit", display: "inline-flex", alignItems: "center", gap: 4 }}
    >
      เข้าสู่ระบบ <ArrowIcon />
    </button>
  )}
  <a className="landing-text-link" href="/app?surface=desktop">
    ดู Desktop surface <ArrowIcon />
  </a>
</div>
```

- [ ] **Step 4: Verify TypeScript compiles**

```bash
npx tsc --noEmit
```

Expected: No new errors.

- [ ] **Step 5: Verify Vite build**

```bash
npx vite build
```

Expected: Build succeeds.

- [ ] **Step 6: Verify landing page still loads**

Open `http://localhost:1420/` in the browser preview. Confirm:
- Landing page renders with scroll narrative intact
- "เข้าสู่ระบบ" button appears in header and hero
- No console errors

- [ ] **Step 7: Verify /app route shows AuthGuard**

Open `http://localhost:1420/app` in the browser preview. Confirm:
- Shows LoadingScreen briefly, then redirects to `/` (since no session exists)

- [ ] **Step 8: Verify /app?surface=desktop is NOT gated**

Open `http://localhost:1420/app?surface=desktop` in the browser preview. Confirm:
- Desktop app loads directly without any auth check or redirect

- [ ] **Step 9: Commit**

```bash
git add src/main.tsx src/landing/LandingPage.tsx src/landing/landing.css
git commit -m "feat(auth): add routing, login buttons on landing page, and AuthGuard on /app"
```

---

## Self-Review

### Spec coverage

| Spec Section | Task |
|---|---|
| §1 Auth Flow + Session Management | Task 1 (client), Task 2 (triggers), Task 3 (AuthCallback, AuthGuard), Task 5 (login buttons) |
| §2 Routing | Task 5 (main.tsx RootRouter) |
| §3 AuthGuard | Task 3 |
| §4 AuthCallback | Task 3 |
| §5 Dashboard | Task 4 |
| §6 Account Settings | Task 4 |
| §7 Landing Page Modifications | Task 5 |
| §8 Supabase Client | Task 1 |
| §9 LoadingScreen | Task 3 |
| §10 Supabase Migration | Task 2 |
| §11 Supabase Configuration (Manual) | Not a code task — manual setup in Supabase + Google Cloud Console |
| §12 Environment Variables | Task 1 |
| §13 Security Checklist | Verified: `.env` gitignored ✅, anon key only ✅, RLS exists ✅, PKCE default ✅ |
| §14 File Inventory | All files accounted for in Tasks 1-5 |
| §15 Dependencies | Task 1 installs `@supabase/supabase-js` |

### Placeholder scan

No TBD, TODO, "implement later", or vague steps found.

### Type consistency

- `supabase` export: created in Task 1 `src/lib/supabase.ts`, consumed in Tasks 3, 4, 5 — same import path.
- `LoadingScreen` component: created in Task 3, consumed in Task 3 (AuthCallback, AuthGuard) — same file.
- `AuthCallback`, `AuthGuard`, `Dashboard`: created in Tasks 3-4, consumed in Task 5 `main.tsx` — same named exports.
- `AccountSettings({ onClose })`: created and consumed within Task 4 Dashboard — consistent prop type.
- `User` type from `@supabase/supabase-js`: used in Task 5 `LandingPage.tsx` — correct import.

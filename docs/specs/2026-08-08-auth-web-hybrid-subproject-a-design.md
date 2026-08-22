# Sub-project A: Auth + Login UI — Design Spec

> **Parent project**: Auth + Web Hybrid Architecture for FUNG
> **Scope**: Google OAuth login via Supabase, web dashboard shell, account settings
> **Out of scope**: Device pairing (B), cloud storage config (C), desktop login (D), mobile login (E), BYOM API keys (F)

## 1. Auth Flow + Session Management

### Login flow

1. User clicks "เข้าสู่ระบบ" on landing page (header, hero, or closing CTA)
2. Call `supabase.auth.signInWithOAuth({ provider: 'google', options: { redirectTo: '<origin>/auth/callback' } })`
3. Browser redirects to Google consent screen
4. Google redirects back to Supabase (`https://nqnrvqnijzovkrhxslfp.supabase.co/auth/v1/callback`)
5. Supabase redirects to `/auth/callback#access_token=...&refresh_token=...`
6. Supabase JS client reads hash tokens automatically, stores in localStorage
7. `AuthCallback` component detects session → `window.location.href = '/app'`
8. `AuthGuard` wrapping Dashboard checks session → renders Dashboard

### Profile provisioning

Database trigger on `auth.users` INSERT:
- Creates `profiles` row with `display_name` from Google metadata (`full_name` or `name`, fallback `'User'`)
- Creates `oauth_connections` row with `provider = 'google'`, `status = 'active'`, `approved_scopes = ['openid', 'email', 'profile']`
- Both triggers use `SECURITY DEFINER` to bypass RLS

### Session management

- `@supabase/supabase-js` manages localStorage tokens and auto-refresh
- `AuthGuard` uses `onAuthStateChange` listener to react to token expiry mid-session
- No custom token handling — Supabase SDK handles everything
- PKCE flow is default in `@supabase/supabase-js` v2 for OAuth

### Logout

- `supabase.auth.signOut()` → clears localStorage → redirect to `/`

## 2. Routing

FUNG uses conditional rendering by `window.location.pathname` — no React Router.

### Route table

| Path | Condition | Component | Auth required |
|------|-----------|-----------|---------------|
| `/` | — | `LandingPage` | No |
| `/app` | `surface=desktop` or `surface=mobile` | Existing app (unchanged) | No |
| `/app` | No surface param | `AuthGuard` → `Dashboard` | Yes |
| `/auth/callback` | — | `AuthCallback` | No (processes tokens) |

### Key constraint

`/app?surface=desktop` and `/app?surface=mobile` are **not gated by auth**. They continue to work exactly as before — local-first, no account required. Auth gate applies only to the web dashboard (`/app` without surface param).

### Route resolution in App.tsx

```typescript
const path = window.location.pathname;
const params = new URLSearchParams(window.location.search);
const surface = params.get('surface');

if (path === '/app' && (surface === 'desktop' || surface === 'mobile')) {
  return <ExistingApp />;  // unchanged
}
if (path === '/auth/callback') {
  return <AuthCallback />;
}
if (path === '/app' && !surface) {
  return <AuthGuard><Dashboard /></AuthGuard>;
}
return <LandingPage />;
```

## 3. AuthGuard

`src/web/AuthGuard.tsx`

Three states: `loading` → `authenticated` → render children, or `unauthenticated` → redirect to `/`.

- **Loading**: Shows `LoadingScreen` (FUNG logo + spinner). Duration is minimal — just a localStorage token check.
- **Authenticated**: Renders children.
- **Unauthenticated**: `window.location.href = '/'` redirect.
- Subscribes to `onAuthStateChange` to handle mid-session token expiry.

## 4. AuthCallback

`src/web/AuthCallback.tsx`

Handles the OAuth redirect from Supabase:

1. Renders `LoadingScreen` with message "กำลังเข้าสู่ระบบ..."
2. Calls `supabase.auth.getSession()` — Supabase JS reads hash tokens automatically
3. On success: `window.location.href = '/app'`
4. On error: Shows error message with link back to `/`

## 5. Dashboard

`src/web/Dashboard.tsx`

Post-login landing page — a shell with placeholder content.

### Layout

```
┌─────────────────────────────────────────────┐
│  FUNG ☁️                        [avatar] ▼  │
│─────────────────────────────────────────────│
│                                             │
│      ยินดีต้อนรับสู่ FUNG Web               │
│      (display_name from profiles)           │
│                                             │
│      ┌─────────────┐  ┌─────────────┐      │
│      │ 🎙️ เริ่มบันทึก │  │ 📁 ไฟล์ล่าสุด │      │
│      └─────────────┘  └─────────────┘      │
│                                             │
│      (placeholder tiles — not functional    │
│       until sub-project B+)                 │
│                                             │
└─────────────────────────────────────────────┘
```

### Top bar

- FUNG logo + cloud icon (web mode indicator)
- Avatar dropdown menu:
  - `ตั้งค่าบัญชี` → opens AccountSettings panel
  - `ออกจากระบบ` → `supabase.auth.signOut()` → redirect to `/`

### Data fetching on mount

```typescript
supabase.auth.getUser()           → email, avatar_url
supabase.from('profiles').select() → display_name
```

### Placeholder tiles

Static cards showing UI structure. Not functional until sub-project B adds the compute layer.

## 6. Account Settings

`src/web/AccountSettings.tsx` + `src/web/AccountSettings.css`

Slide-over panel following the existing `TtsProviderPanel` / `ExternalAccountPanel` pattern.

### Sections

**Active in sub-project A:**

1. **โปรไฟล์** — Read/edit `display_name` from `profiles` table. Email from `auth.getUser()` (read-only).
   - Save: `supabase.from('profiles').update({ display_name })` — RLS ensures user can only update own row.
2. **บัญชีที่เชื่อมต่อ** — Shows Google as connected (because login is via Google). Reads from `oauth_connections` table.

**Placeholder sections (disabled, showing "ยังไม่พร้อมใช้งาน"):**

3. **Cloud Storage** — Sub-project C will add Google Drive / OneDrive / S3 configuration.
4. **อุปกรณ์ที่จับคู่** — Sub-project B will add device pairing + FUNGWIRE tunnel.

### CSS pattern

Hardcoded light-theme values + `.theme-dark .account-*` overrides, matching the existing `ExternalAccountPanel` / `ZoomPanel` / `TtsProviderPanel` convention.

## 7. Landing Page Modifications

Minimal changes to `src/landing/LandingPage.tsx` — add login button, show avatar when logged in.

### Session check

Call `supabase.auth.getSession()` once on mount. No redirect, no blocking, no loading spinner. Fallback to logged-out state if Supabase is unreachable.

### Header changes (line 86-104)

- **Not logged in**: Add "เข้าสู่ระบบ" button before "เปิด FUNG" CTA. Calls `signInWithOAuth`.
- **Logged in**: Replace login button with avatar thumbnail + dropdown (ตั้งค่าบัญชี → `/app` with settings open, ออกจากระบบ → signOut + reload).
- "เปิด FUNG →" button unchanged — works both logged-in and logged-out.

### Hero actions (line 121-131)

- **Not logged in**: Add "เข้าสู่ระบบด้วย Google" button with Google G icon between primary CTA and APK download.
- **Logged in**: Hide this button.

### Closing CTA (line 426-448)

- **Not logged in**: Add "เข้าสู่ระบบ →" link after "เปิด FUNG" button.
- **Logged in**: Hide this link.

### Unchanged

- Scroll narrative / animations
- Architecture section
- Footer
- "ใช้งานแบบ Local ได้โดยไม่ต้องมีบัญชี" message (still true — desktop doesn't require login)

## 8. Supabase Client

`src/lib/supabase.ts`

```typescript
import { createClient } from '@supabase/supabase-js';

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL;
const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY;

export const supabase = createClient(supabaseUrl, supabaseAnonKey);
```

- Uses Vite env vars (`VITE_` prefix)
- Anon key only — no service role key in frontend
- Single instance, imported wherever needed

## 9. LoadingScreen

`src/web/LoadingScreen.tsx` + `src/web/LoadingScreen.css`

Shared component used by AuthGuard and AuthCallback:

- FUNG logo (via `FungLogo` component)
- Subtle spinner
- Optional message prop

## 10. Supabase Migration

`supabase/migrations/20260808000000_profile_trigger.sql`

### Profile auto-provisioning trigger

```sql
CREATE OR REPLACE FUNCTION public.handle_new_user()
RETURNS trigger AS $$
BEGIN
  INSERT INTO public.profiles (id, display_name)
  VALUES (
    NEW.id,
    COALESCE(NEW.raw_user_meta_data->>'full_name', NEW.raw_user_meta_data->>'name', 'User')
  );
  RETURN NEW;
END;
$$ LANGUAGE plpgsql SECURITY DEFINER;

CREATE TRIGGER on_auth_user_created
  AFTER INSERT ON auth.users
  FOR EACH ROW EXECUTE FUNCTION public.handle_new_user();
```

### Google OAuth connection tracking

```sql
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

## 11. Supabase Configuration (Manual)

### Google OAuth provider

Enable in Supabase Dashboard → Authentication → Providers → Google:
- Client ID and Client Secret from Google Cloud Console OAuth 2.0 Client
- Redirect URL: `https://nqnrvqnijzovkrhxslfp.supabase.co/auth/v1/callback`

### Google Cloud Console

Create OAuth 2.0 Client ID:
- Application type: Web application
- Authorized JavaScript origins: `https://fung.dev`, `http://localhost:1420`
- Authorized redirect URIs: `https://nqnrvqnijzovkrhxslfp.supabase.co/auth/v1/callback`

### Supabase URL Configuration

- Site URL: `https://fung.dev`
- Redirect URLs allow list: `https://fung.dev/auth/callback`, `http://localhost:1420/auth/callback`

## 12. Environment Variables

### Development (`.env` — gitignored)

```
VITE_SUPABASE_URL=https://nqnrvqnijzovkrhxslfp.supabase.co
VITE_SUPABASE_ANON_KEY=<anon-key>
```

### Production (Vercel Environment Variables)

Same keys, set via Vercel dashboard or `vercel env`.

### Template (`.env.example` — committed)

```
VITE_SUPABASE_URL=https://your-project.supabase.co
VITE_SUPABASE_ANON_KEY=your-anon-key
```

## 13. Security Checklist

| Item | Status | Detail |
|------|--------|--------|
| RLS on profiles | ✅ exists | User reads/updates only own row |
| RLS on oauth_connections | ✅ exists | User reads only own rows |
| RLS on devices | ✅ exists | User reads only own rows |
| No service role key in frontend | ✅ | Anon key only |
| Tokens not stored in app DB | ✅ | Google tokens in Supabase Auth internal only |
| PKCE flow | ✅ | Default in @supabase/supabase-js v2 |
| `.env` in `.gitignore` | ⚠️ verify | Must confirm before implementation |
| Anon key exposure | ✅ safe | Public by design — RLS is security boundary |
| XSS protection | ✅ | React escapes HTML; tokens in localStorage |
| CSRF protection | ✅ | PKCE + state parameter (Supabase managed) |
| Session auto-refresh | ✅ | Supabase JS handles token refresh |
| Desktop app not gated | ✅ | `/app?surface=desktop` bypasses AuthGuard |

## 14. File Inventory

### New files (8)

| File | Purpose |
|------|---------|
| `src/lib/supabase.ts` | Supabase client singleton |
| `src/web/AuthGuard.tsx` | Session check + redirect wrapper |
| `src/web/AuthCallback.tsx` | OAuth callback handler |
| `src/web/Dashboard.tsx` | Post-login web dashboard shell |
| `src/web/AccountSettings.tsx` | Account settings slide-over panel |
| `src/web/AccountSettings.css` | Settings panel styles |
| `src/web/LoadingScreen.tsx` | Shared loading component |
| `.env.example` | Environment variable template |

### Modified files (3)

| File | Change |
|------|--------|
| `src/App.tsx` | Route branching for `/auth/callback` and web `/app` |
| `src/landing/LandingPage.tsx` | Login/avatar in header + hero + closing CTA |
| `package.json` | Add `@supabase/supabase-js` |

### Supabase migration (1)

| File | Purpose |
|------|---------|
| `supabase/migrations/20260808000000_profile_trigger.sql` | Profile + OAuth connection auto-provisioning triggers |

### CSS additions

- `src/landing/landing.css` — `.login-btn`, `.avatar-btn`, `.avatar-dropdown` styles
- `src/web/LoadingScreen.css` — Loading screen styles

### Manual configuration (not code)

- Enable Google provider in Supabase Dashboard
- Create OAuth Client in Google Cloud Console
- Set Vercel environment variables
- Verify `.env` in `.gitignore`

## 15. Dependencies

### New npm dependency

```
@supabase/supabase-js ^2.x
```

### Existing dependencies used

- `react`, `react-dom` — UI
- `lucide-react` — icons (LogIn, User, ChevronDown, etc.)
- `FungLogo` component — loading screen, dashboard

### No new Rust dependencies

Sub-project A is frontend + Supabase only. No Tauri backend changes.

---
version: "0.1.0b"
created_at: "2026-09-21T03:36:16+07:00,RWANG,base-b336f33"
last_update: "2026-09-21T03:58:31+07:00,RWANG"
status: "candidate"
superseded_by: null
attributes:
  domain: "meeting-platform-adapters"
  doc_type: "architecture-decision"
  scope: "Google Meet API speaker attribution and meeting-agent delivery"
  language: "Thai"
---

# ADR — Google Meet API-first Meeting Agent

## 1. Decision status

User direction on 2026-09-21: **Google Meet first; API acceptable to help separate speakers**.
This ADR proposes the implementation route; vendor, billing, region, remote retention and deployment are **not approved/activated by this document**. No paid service or meeting was accessed.

C-3 / HIGH. Parents: [Domain map](../architecture/MEETING_INTELLIGENCE_DOMAINS.md), [Desktop architecture](../Desktop/ARCHITECTURE.md).
Contracts: [Live transcription](../specs/2026-09-21-live-meeting-transcription-spec.md), [Agent](../specs/2026-09-21-meeting-agent-participation-spec.md).

## 2. Decision

1. Use an adapter contract separating media/participant input from chat/link/file/audio output.
2. For the complete “join Meet + answer in Meet chat” feature, qualify a **managed meeting bot API** first. **Recall.ai is a candidate reference provider**, not a procurement commitment or already-integrated dependency.
3. Keep official Google Meet Media API as an optional direct media/metadata adapter for eligible environments. Do not claim it supplies chat posting or conversational audio output.
4. Preserve local mic/system capture as independent local-only mode. No automatic transfer to a cloud bot when official API eligibility fails.
5. Prefer source-bound participant identity over speculative biometric attribution. Keep pyannote and optional reviewed voice matching for mixed/shared-room/unknown cases.

## 3. Verified documentation snapshot — 2026-09-21

### 3.1 Official Google APIs

| Surface | Documented capability | FUNG interpretation |
| --- | --- | --- |
| Meet Media API | real-time audio/video and participant metadata; Developer Preview requiring project, OAuth principal and all participants enrolled | eligibility is a hard preflight/release gate, not generally available production access |
| MediaEntry | audioCsrc, participant resource/key, session/sessionName | map source packet/span to session; don't use deprecated participantId as the new stable contract |
| C++ media reference | conference media receive-only; no media sending into conference | voice reply capability false for this route |
| Meet artifacts REST | generated transcript/recording artifacts and transcript entries | separate artifact import/reconciliation, not evidence of live token streaming |
| Meet add-on SDK | side panel/main-stage collaborative app surfaces | useful optional evidence UI, not equivalent to bot chat delivery |
| Google Chat API | messages in Chat spaces/threads | separate destination from Meet in-call chat; no silent replacement |

Sources: [Media overview](https://developers.google.com/workspace/meet/media-api/guides/overview), [MediaEntry](https://developers.google.com/workspace/meet/media-api/reference/dc/media_api.mediaentry), [receive-only reference](https://developers.google.com/workspace/meet/media-api/reference/cpp/namespace/meet), [artifacts](https://developers.google.com/workspace/meet/api/guides/artifacts), [add-on deployment](https://developers.google.com/workspace/meet/add-ons/guides/deploy-add-on), [Chat messages](https://developers.google.com/workspace/chat/create-messages).

Minimum proposed direct-media scopes are `meetings.conference.media.audio.readonly` and `meetings.space.read`; only request broader media/video scopes if a separate feature needs them. Restricted-scope verification/security assessment obligations depend on actual data handling and must be reviewed before release. Codec/header/client requirements must be checked even for an audio-focused application, not inferred away. [Google setup requirements](https://developers.google.com/workspace/meet/media-api/guides/get-started)

**Negative-capability boundary:** no supported Meet in-call chat-send/file-upload method was established in the official surfaces inspected. This is a checked design limitation, not a claim that no future Google API can ever support it. Generic WebRTC data-channel capability does not authorize arbitrary Meet chat messages or file transfer. Recheck official API capabilities at implementation/qualification.

Official virtual audio streams can change their contributing participant; use CSRC/media-entry mapping rather than stream-slot identity. A finite relevant-stream selection is not guaranteed lossless audio from every attendee. [Media concepts](https://developers.google.com/workspace/meet/media-api/guides/concepts)

### 3.2 Managed bot candidate: Recall.ai

| Capability | Documented basis | Product boundary |
| --- | --- | --- |
| Google Meet bot participant | create a bot associated with a meeting; visible participant behavior | host admission and tenant policy still apply |
| Separate realtime audio | Meet supported; currently up to 16 concurrent loudest speakers; PCM S16LE mono 16kHz through WebSocket | not arbitrary attendee count; realtime separate streams exclude screenshare audio |
| Chat input/output | Meet receiving and sending supported; send recipient is everyone, current limit 500 characters | no per-participant Meet DM; actual-room delivery must be proven |
| Dynamic audio output | conversational use routed through Output Media, not the short-clip Output Audio endpoint | optional follow-up capability; separate implementation/gates |
| Native file attachment | not established by sources inspected | capability false until independently proven; MVP uses authorized link |

Sources: [bot model](https://docs.recall.ai/docs/bot-overview), [separate participant audio](https://docs.recall.ai/docs/how-to-get-separate-audio-per-participant-realtime), [chat input](https://docs.recall.ai/docs/receiving-chat-messages), [chat output](https://docs.recall.ai/docs/sending-chat-messages), [audio output guidance](https://docs.recall.ai/docs/output-audio-in-meetings), [short-clip endpoint limitations](https://docs.recall.ai/reference/bot_output_audio_create).

Realtime audio uses a publicly reachable WebSocket server; this is not the Desktop dialing an ordinary provider audio-download URL. Build/approve a gateway with an outbound Desktop subscription. Authenticate provider requests using documented signatures and appropriate account-generation secrets, not a guess at a header or a token pasted into public logs. [WebSocket endpoints](https://docs.recall.ai/docs/real-time-websocket-endpoints), [request verification](https://docs.recall.ai/docs/authenticating-requests-from-recallai)

No price, retention guarantee, SLA, supported account type or bot-delete guarantee is inferred from these capability pages. Obtain exact configuration/terms and test actual account behavior before using real meeting data.

## 4. Candidate capability matrix

Values are design expectations to validate, **not current FUNG support**.

| Capability | Local capture | Official Meet media | Managed bot candidate |
| --- | --- | --- | --- |
| Audio input | mic/system | receive-only API, preview-gated | participant-separated input via gateway |
| Participant-source label | channels only | CSRC + MediaEntry/session | provider participant ID + join/session generation |
| Visible autonomous bot | no | media app connection semantics; qualify UX | documented participant; qualify admission |
| Live local Whisper | existing chunked basis | proposed | proposed |
| Meet chat read/send | no | not established | documented, still needs real-room proof |
| Authorized link in Meet chat | manual operator only | not established | proposed through chat text |
| Native file attachment | no | not established | not established |
| Speak into Meet | no automatic loopback | unsupported on inspected media route | optional Output Media qualification |
| Operate fully offline | yes | no | no |
| Additional public gateway | no | not assumed necessary | required for realtime ingress route |

Google Chat file upload has its own user-authentication scopes and attachment API; it does not prove Meet file attachments or app-auth attachment upload. If later selected as a different output channel, implement a distinct binding/auth flow. [Chat media upload](https://developers.google.com/workspace/chat/api/reference/rest/v1/media/upload)

## 5. Adapter contracts — GM requirements

| ID | Contract |
| --- | --- |
| GM-01 | `probeCapabilities` returns provider/platform/version/account-specific allowed operations, limits, preview/tenant blockers and evidence timestamp |
| GM-02 | `prepare/join/status/leave` are bounded session operations; report accepted, admitted, receiving and ended separately |
| GM-03 | Normalize audio frames with occurrence/session/participant/track generation, sequence, media time, codec and source ID; preserve gaps |
| GM-04 | Resolve speaker labels from provider metadata without equating them to a confirmed FUNG Person |
| GM-05 | `publishText/publishLink` enforce destination binding and documented limit; `publishFile/speak` return unsupported unless qualified |
| GM-06 | Verify authenticated incoming events; normalize duplicates, out-of-order metadata and reconnect generations |
| GM-07 | Return receipts with external IDs and accepted/delivered/unknown distinction; do not promise exactly-once without provider support |
| GM-08 | Expose stop, retention/delete status, cost counters and cleanup failures; network/account errors cannot silently switch providers |

No vendor-specific request JSON is the public FUNG domain API. Store adapter version and source docs date in run manifests. Pin tested SDK/protocol versions; do not build from a floating example repository head.

## 6. Participant attribution and People integration

Normalized key: provider account + conference occurrence + participant session/join generation. Provider ID scope is preserved; no cross-meeting merge from same display name.

- UI may show “คุณเมย์ · ชื่อจาก Meet” automatically, with source badge.
- Optional user confirmation links this session to a local Person; only that review creates a confirmed-person overlay.
- Rejoin creates new session/generation; explicit provider stable IDs may support suggestions, not unreviewed human identity proof.
- Same room-device account can contain multiple actual speakers. Retain parent endpoint and use anonymous diarization children; never tell user API resolves all shared-mic speakers.
- An agent participant is `kind=agent`, excluded from enrollment/candidate human gallery and default conversation triggers.
- Unknown/delayed mapping is accepted as unknown; audio/transcript continues. Audio missing because of source limits stays a gap, not silent completeness.

## 7. Deployment, privacy and choices still needed

Managed route adds a public transport component described in the [Agent gateway contract](../specs/2026-09-21-meeting-agent-participation-spec.md#12-deployment-and-gateway-contract). Local ASR/knowledge remain on Desktop, but meeting media already passes through the managed bot vendor. That mode must be visibly **online / external meeting media**, never marketed as all-local.

Before implementation enablement:

1. Owner approves provider, region, pricing cap, retention/delete settings and gateway operation.
2. Configure Google/tenant admission and notices; official API needs preview/scope eligibility if selected.
3. Choose runtime supporting long-lived WSS and watchdog; approve public endpoint/TLS/secret custody.
4. Decide authorized artifact host for local document excerpts, or use already accessible source links only.
5. Validate user/account/guest/chat settings, no-gateway/low-bandwidth failure and actual stop behavior.
6. Check legal/privacy requirements with the deployment owner; this spec is a control design, not a legal-compliance certification.

No credentials should be requested in conversation; setup uses keyring/server secret store after implementation approval. No calendar read scope or domain-wide delegation is needed by default for manually selected meeting URLs.

## 8. Acceptance / go-no-go

- Real Google Meet: two remote speakers, overlap, one shared-mic room, rename/rejoin and one guest. Verify source label accuracy and explicitly measure missing/misattributed duration.
- Thai/English local Whisper with source timestamps and full model provenance; no hidden external transcription.
- Agent receives question, finds approved exact report, posts cited short answer/link in the **same Meet chat**; another participant opens the authorized link.
- Negative test: chat disabled, participant limit, screenshare audio, anonymous audience, invalid signature, expired session and uncertain send.
- Stop/revoke/desktop disconnect/gateway crash: verify bot leaves or reports unresolved remote state, no new message, and independent local capture behavior.
- Native attachment and spoken reply remain separate unchecked gates; meeting-chat screenshot cannot satisfy either.
- Record API account/version/region, actual tenant settings, device/runtime, costs and result. Fixtures alone never close provider qualification.

If managed API fails qualification or vendor is not approved, ship local observe/draft scope only and leave the full participation feature open. Do not represent this fallback as fulfilling automatic in-meeting response.

## Version Diff

| Version | Change |
| --- | --- |
| 0.0.0 → 0.1.0b | Proposed Google Meet-first adapter strategy with official API limits, managed-bot candidate and direct speaker attribution. |

## CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
| --- | --- | --- | --- | --- | --- |
| 0.1.0b | 2026-09-21 | candidate | Source-checked API feasibility and qualification gates; no vendor activation. | working-tree; base b336f33 | RWANG |

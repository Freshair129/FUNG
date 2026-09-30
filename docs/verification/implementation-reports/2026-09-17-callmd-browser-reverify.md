---
version: "0.1.4b"
created_at: "2026-09-17T09:11:00+07:00,Codex,376ef30db13670e4dea816ceff440f44ce73fffd"
last_update: "2026-09-17T20:56:22+07:00,Codex"
status: "under review"
superseded_by: null
attributes:
  doc_type: "verification-report"
  scope: "Main-orchestrator actual browser observation; no product code edits"
---

# Browser re-verification after integrated lifecycle RCA

CURRENT RESULT: scoped FIX3 browser re-verification PASS on CI144 merged source;
the earlier browser-attach failure is no longer the current observation. Native,
packaged persistence/audio and broader keyboard/device acceptance remain open.
Prior pauses/failures below are retained as historical evidence, not current CI
results. CI144 separately passed; it does not constitute native runtime proof.

Story: desktop Home/Live/History renders truthful capture state and preserves one
Live owner plus recording-scoped review; native transport remains a separate gate.
Verification skills require actual UI evidence and stopping at a broken boundary.
Agent-browser CLI was unavailable; CUA used without installing dependencies.

## Pinned package-checkout browser recheck — 2026-09-17 20:51-20:56 ICT

Observed by main orchestrator, not inferred from worker text. Source is clean
detached merge `3aafe1e5efe1cbfe2852cb73cf20140f12841f90`, tree
`5ddbc23f9fa3484e7b9351d7f1981219ed9d2775`, under
`output/callmd-desktop-ci144-3aafe1e`. Original source/test pins below are
unchanged. No implementation, test, config, bridge or native state was edited.

- The existing Live fixture ran from that checkout at127.0.0.1:5173, session93322.
  Actual AX text and screenshot showed FIXTURE_COMPLETE and all seven PASS rows:
  hidden0x0/displaynone; one visible owner1280x800; three light labels
  rgb48,56,51; three dark labels rgb244,241,234; StrictMode ready/inactive with
  two registrations; honest NATIVE_UNAVAILABLE; unmounted controller rejection
  LEGACY_COMMAND_FAILED. Console warn/error list was empty.
- Actual App used unchanged Vite through installed dependencies at
  127.0.0.1:15475/app?surface=desktop, session34714. No optional env file was
  copied; no Tauri internals were injected. Home -> Live -> History -> Live ->
  Home navigation worked; focus moved to the corresponding main heading.
- Home disclosed native project inventory unavailable, not fabricated rows.
  Live disclosed NATIVE_UNAVAILABLE, kept capture/Q&A/summary disabled, and
  History required a project/recording without starting capture/playback.
- Both History themes were observed in screenshots. Live's three form labels
  measured rgb48,56,51 in light and rgb244,241,234 in dark, opacity1 and a dark
  ancestor present. App's first new tab defaulted to1280x720 despite the earlier
  fixture override; the override was reapplied and final Live light/dark and
  History observations explicitly measured1280x800. No full all-state clipping
  or broad keyboard acceptance is implied.
- Returning Home hid Live with hidden=true, displaynone and0x0. Captured App
  console warn/error list was empty. A getByLabel theme locator had no matches;
  fresh DOM grounded the working combobox-role locator. This was an automation
  locator correction, not a source defect or a silently retried UI failure.
- Both owned tabs closed, original light restored, viewport override reset;
  both owned server sessions stopped. No listeners remained on5173/15475.
  Only task-generated Temp/fung-callmd-live-fixture-ksDaNF was deleted after
  exact canonical immediate-child and root/descendant reparse checks; absence
  verified. Other user tabs/processes/files were untouched.

This closes the bounded FIX3 contrast/navigation re-observation only. Native
boot/capture/playback, real persisted History, restart, provider, OS-theme
transition and complete keyboard/retained-surface journeys remain NOT_RUN.
Package preparation has a separate missing Python3.11+pip prerequisite and an
unspecified credential-isolated Windows account/VM; neither is bypassed.

## Approved FIX3 browser attempt — 2026-09-17 12:34-12:36 ICT

Version diff0.1.2b ->0.1.3b records the newly approved source and an environment
blocker, not a visual PASS or a new application defect. Source is frozen:
CSS99cecc78ca46f1670cbfe062b877071973405a55f3a4f8a7b638c94a8703d61e;
test0552e2cc5375dea5a3d48020e44c0ed6e1959df261ecba8ec4abd095f4d423b2.
App remains d26064747c604dc0d350160e7ccebeb881ecc745557cbd35cfa19d4266334039.

- `node tests/callmdLiveWorkspace.test.mjs --fixture` started successfully:
  session69763, URLhttp://127.0.0.1:5173/, seven expected PASS rows. Startup is
  not evidence that those rows passed.
- CUA `createBrowserTab` twice and documented `browser.tabs.new` fallback each
  timed out waiting for the Browser webview to attach. Browser inventory was
  empty. No page DOM, screenshot, console, computed color, or fixture result
  was observed. Actual integrated App browser check is NOT_RUN.
- Browser troubleshooting guidance was followed; no alternate automation
  framework, browser install, native launch, or runtime injection was used.
  Verification stopped at the unavailable observation boundary.
- The temporary1280x800 viewport override was reset. No created tab remained.
  Server69763 was stopped; port5173 has zero listeners.
- Exact generated Temp/fung-callmd-live-fixture-NGhKpl was resolved, verified as
  an immediate task-prefixed system-temp child, checked for root/descendant
  reparse points, removed, and absence verified. Only reproducible generated
  fixture/cache files were deleted; source and user data were retained.

Next verification requires an available Codex browser: run the existing fixture
and observe all seven rows, then inspect actual App light/dark labels and
Home/Live/History navigation. No additional source change is implied by the
tooling failure. Native/audio/Whisper/strictClippy/hostedCI remain separate gates.

## Live FIX2 component — PASS 5/5

At09:08ICT main ran the accepted isolated Live fixture and observed actual AX,
screenshot and empty warning/error logs, not merely the worker's report.
Command: node tests/callmdLiveWorkspace.test.mjs --fixture in live-376ef30.
Observed URL127.0.0.1:5174, completion marker FIXTURE_COMPLETE, five visible PASS:

- Hidden owner: hidden=true, display=none, rectangle0x0.
- Visibility toggle: display=flex, rectangle1280x720, exactly one owner.
- StrictMode replay: two registrations, published ready/active=false/phase=idle.
- Native-absent notice remains visible; status escaped loading.
- Genuine unmount: retained controller rejects LEGACY_COMMAND_FAILED.

Pinned PanelTSX c9b4599d1b74c7f88898013de51373cdd682924de2850a477bf2ef6baa93ac8c;
PanelCSS6cd16f22820df076773420fca624507fc78daa05145ff4d3711243258bd11c1f;
testd77d811c1c6b2b9056f40d3ecc77f462c2455e9aa411c843aedc6692ffe9fd49.
Real ReactDOMClient/StrictMode and production component, native-absent browser;
no Tauri injection, real recording, provider or device proof.

Own tab closed, server session10703 stopped, port5174 absent. Exact generated
Temp/fung-callmd-live-fixture-sDOe3k canonicalized as immediate task-prefixed temp
child, no reparse points, removed and absence verified. Only generated fixture/
cache files were deleted; source/user data retained.

## Integrated App and History exact-count reproof

IntegrationFIX1 sourcefreeze App3efb66d9e71b6158eec9e5c0753d7dbcd6c132ac9a186bc40d24c79a9d5fc804,
test9f015b97e158e65220ba9c2db1f952861ebaa6bd231dea6d268d0c8eb0b0e298.
Main actual App at127.0.0.1:15473/app?surface=desktop,1280x800:

- PASS Home displays ready/unavailable truthfully, not preparing/active capture.
- PASS hiddenLive and hiddenHistory both displaynone/0x0; no consolewarn/error.
- PASS navigation Home -> Live -> History and focus on route heading.
- PASS Live explicitly shows NATIVE_UNAVAILABLE and disabled capture/Q&A.
- PASS Settings entry reachable; no sign-in/provider/account operation.
- FAIL dark theme: Shell background23,25,24; Review250,248,243, no.theme-dark
  ancestor. Screenshot confirms Shell dark/Review light. Stopped at this boundary.
  Theme restored to initial light, own tab/server closed, viewport reset.
  Port15472 was occupied; did not kill its owner. Task used15473, now released.

RCA callmd-integrated-theme-scope.md; Pauli independently confirmed missing owner
theme ancestor affects bothLive andHistory by source. Fresh Luna Rawls FIX2 now
owns narrowApp/test correction. No full-App/native acceptance granted.

Separately, unaffected actual production RecordingReview fixture was rerun on the
strict9f015test hash: FIXTURE_COMPLETE, sixvisiblePASS, consolewarn/error[]. Current
pair now uses equality baseline+1 and actualdelta1; unmountretainedBlist/release0/0.
Pauli already confirmed remainingfive per-casebooleans (later displayed aggregate
deltas are not the assertions). This closes component exact-count proof only.
Own fixture session64869/tab closed; exact tempfung-callmd-react-fixture-9uVUZ6
canonicalized as task-prefixed immediate tempchild/no reparse, removed; absence
and port5174release verified. Only generated fixture/cache, no user data removed.

## IntegrationFIX2 final browser checkpoint

Appd26064747c604dc0d350160e7ccebeb881ecc745557cbd35cfa19d4266334039,
testfda7b3d858f26bf4247319161911eda92fb30ebba20409d3bfcb5a5eb7069c35.
Actual127.0.0.1:15475/app?surface=desktop,1280x800, sourcelease released:

- Home hidden Live/History still displaynone/0x0, truthful no-active state.
- Home->History->Live navigation remains available with heading focus.
- History dark PASS: darkancestor present, backgroundrgb23,25,24, readable cards.
- Live theme ancestry PASS: darkancestor present, darkcapturecardrgba34,38,36,.92.
- New FAIL: enabled consent/system-capture and language labels retain rgb48,56,51
  atopacity1 on that darkcard. Source hardcodedlightcolor lacksdarkoverride.
- Console warnings/errors[]; original light restored, tab/server26520 closed,
  viewport reset. No native/audio/account/provider/user-data operation.

See RCA callmd-live-dark-label-contrast.md. Per agent-browser-verify maximumtwo
automatic retries, stopped at this boundary. Actual OS-dark/system-transition
browser observation, broader keyboard/pairing/recovery/nativejourney NOT_RUN.
Deterministic theme-helper transition evidence is separate, never OS observation.
Previous strictHistory6PASS is pinned to9f015test and unchanged productionReview;
reviewer must verify fixture logic unchanged underfda7 before carrying it forward.

## Open gates

Native cold boot/device/audio/persistence restart and packaged runtime/provider
remain NOT_RUN. No safe data-and-credential isolated native environment selected.
Browser cannot close those gates. The prior fullRust Whisper6 failures and strict
Clippy failures below belong to the old local snapshot; current hosted CI144 on
3aafe1e passed471/0/1, custody11 and strictClippy. No new packaged artifact exists.

## Version Diff / CHANGELOG

| Version | Date | Status | Summary | Commit Hash | Agent |
|---|---|---|---|---|---|
| 0.1.4b | 2026-09-17 | under review | Main observed Live fixture7/7 and scoped App theme/navigation PASS on exact CI144 tree; native/package gates retained | Local; tested3aafe1e | Codex orchestrator |
| 0.1.2b | 2026-09-17 | under review | Theme ancestry fixed; pause on actual Live dark-label contrast after two retries | UNCOMMITTED; base376ef30 | Codex orchestrator |
| 0.1.1b | 2026-09-17 | under review | Lifecycle/nav pass, theme boundary fails; strict History fixture6PASS | UNCOMMITTED; base376ef30 | Codex orchestrator |
| 0.1.0b | 2026-09-17 | under review | Main Live fixture proof; integrated and exact-count reproof pending | UNCOMMITTED; base376ef30 | Codex orchestrator |

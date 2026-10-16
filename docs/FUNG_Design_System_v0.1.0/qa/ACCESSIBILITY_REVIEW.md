# FUNG — Accessibility review evidence

Date: 2026-10-16  
Status: **Partial evidence; not an accessibility conformance audit.**

## Scope and method

This review covers the currently open FUNG Desktop Home page at `http://127.0.0.1:1421/app?surface=desktop`. It uses the page accessibility tree and sequential keyboard focus observations from this run. It does not claim whole-application WCAG conformance.

## Observed evidence

| Check | Result | Evidence |
| --- | --- | --- |
| Skip-link structure | Present | Accessibility tree exposes “ข้ามไปยังเนื้อหาหลัก” with target `#desktop-shell-main`; the main heading has that ID. Activation and visual reveal were not checked. |
| Heading structure | Present on Home | Main heading “ฟังทุกเรื่องสำคัญ” is level 1. AI Companion, พื้นที่ในเครื่อง, and กิจกรรมโครงการ are exposed as level 2. |
| Control names | Present on Home | The tree exposed names for the meeting, import, review, files, Companion, project, activity-range, and navigation controls. The activity range exposes its selected value. |
| Keyboard focus movement | Partial pass | Fourteen sequential Tab moves from the initial main-heading focus reached ten named buttons in the Home content, followed by four named navigation links: Home, live meeting, history, and files. |
| Visual focus indicator | Not verified | Browser screenshot capture timed out twice in this run, so the visible ring, clipping, and contrast could not be inspected. |

## Findings and limits

No missing accessible name or broken sequential focus transition was observed in the sampled Home sequence. This is a narrow, positive observation; it does not show that the rest of the application meets accessibility requirements.

The review did not evaluate modal focus trapping/restoration, skip-link activation, transcript editing, recording controls, error and recovery flows, light/dark rendering, translucent contrast, text enlargement, forced colors, reduced motion, mobile touch targets, native window behavior, or screen-reader announcements. No screenshot-based review or assistive-technology session was completed.

## Remaining acceptance work

Repeat with a working screenshot capture and cover the important routes and modal states. Check 200% text enlargement, reduced motion, forced colors, and a screen reader; verify native focus and mobile touch sizing on their target runtimes. Keep the result partial until those observations are recorded.

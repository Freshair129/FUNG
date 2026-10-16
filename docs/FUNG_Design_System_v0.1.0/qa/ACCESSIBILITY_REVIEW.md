# FUNG — Accessibility review evidence

Date: 2026-10-16  
Status: **Partial evidence; not an accessibility conformance audit.**

## Scope and method

This review covers the FUNG Desktop Home page at `http://127.0.0.1:4173/app?surface=desktop` from the local production build, plus the recorded sequential keyboard observations. It does not claim whole-application WCAG conformance.

## Observed evidence

| Check | Result | Evidence |
| --- | --- | --- |
| Skip-link structure | Present | Accessibility tree exposes “ข้ามไปยังเนื้อหาหลัก” with target `#desktop-shell-main`; the main heading has that ID. Activation and visual reveal were not checked. |
| Heading structure | Present on Home | Main heading “ฟังทุกเรื่องสำคัญ” is level 1. AI Companion, พื้นที่ในเครื่อง, and กิจกรรมโครงการ are exposed as level 2. |
| Control names | Present on Home | The tree exposed names for the meeting, import, review, files, Companion, project, activity-range, and navigation controls. The activity range exposes its selected value. |
| Keyboard focus movement | Partial pass | Fourteen sequential Tab moves from the initial main-heading focus reached ten named buttons in the Home content, followed by four named navigation links: Home, live meeting, history, and files. |
| Static Desktop Home screenshot | Partial pass | One 1280×720 screenshot from the local production preview shows the header, navigation rail and Home content rendered. It is a single viewport and route. |
| Visual focus indicator | Present; contrast not verified | The Home heading receives programmatic focus and its outline is visible in the screenshot. The screenshot does not establish color contrast, all focus states or behavior at other sizes. |

## Findings and limits

No missing accessible name or broken sequential focus transition was observed in the sampled Home sequence. The production-preview screenshot confirms that the route and a focused heading render; it is a narrow observation and does not show that the rest of the application meets accessibility requirements.

The review did not evaluate modal focus trapping/restoration, skip-link activation, transcript editing, recording controls, error and recovery flows, light/dark comparison, translucent contrast, text enlargement, forced colors, reduced motion, mobile touch targets, native window behavior, or screen-reader announcements. No assistive-technology session was completed.

## Remaining acceptance work

Extend screenshot review across important routes, both themes and modal states. Check 200% text enlargement, reduced motion, forced colors and a screen reader; verify native focus and mobile touch sizing on their target runtimes. Keep the result partial until those observations are recorded.

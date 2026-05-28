# Homepage Redesign Spec (Editorial AI, Incremental)

Date: 2026-05-28
Status: Draft for review
Scope: `server/static/home/index.html`, `server/static/home/home.css` (and minimal `home.js` only if needed)

## 1. Goal

Redesign the homepage to increase professional brand perception while preserving the current information architecture and behavior.

Primary objective:
- Improve brand and visual quality (`专业感 + 品牌感`) with an AI-product + design-tool tone.

Constraints already agreed:
- Base on current homepage instead of full rewrite.
- Focus first on Hero + Header.
- CTA priority: `Use statichub skill` > `Install CLI` > `Run first deploy`.
- Keep the existing content sections functional and recognizable.

## 2. Non-Goals

- No backend changes.
- No routing/domain/auth behavior changes.
- No major IA reordering of lower-page sections.
- No new complex JS interaction patterns.
- No analytics event renaming unless unavoidable.

## 3. Proposed Approach

Use an **incremental visual redesign** of top-of-page only (Header + Hero), preserving lower sections with light style harmonization.

This balances risk and impact:
- High visual impact where users form first impression.
- Low implementation risk by minimizing structural churn.
- Keeps recent README-aligned content changes intact.

## 4. Information Architecture (Preserved)

Keep current section order:
1. Header
2. Hero
3. Trust row
4. Feature cards
5. How it works
6. Quickstart tabs
7. Deploy example
8. Install tabs

No section removal in this phase.

## 5. Header Design

### 5.1 Visual direction
- Lighter, more editorial header treatment.
- Reduce "button-heavy" feel in nav.
- Keep sticky behavior.

### 5.2 Changes
- Brand wordmark: slightly lower apparent weight and refined spacing.
- Top nav links: cleaner, lighter text treatment; stronger hover clarity.
- Actions:
  - Primary action remains prominent.
  - GitHub action visually demoted to secondary.
- Backdrop and border refined for cleaner depth.

## 6. Hero Design

### 6.1 Content (approved direction)
- Pill: `For AI publishing workflows`
- H1: `From prompt to production URL.`
- Subcopy: `Use statichub skill for auto-publish workflows, with CLI install and deploy always available.`

CTA stack:
- Primary: `Use statichub skill`
- Secondary: `Install CLI`
- Tertiary text action: `Run first deploy`

### 6.2 Visual tone
- Keep light background base.
- Add subtle "Editorial AI" atmosphere:
  - soft radial glows
  - very low-contrast grid texture
- Typography and spacing tuned for premium editorial rhythm.

### 6.3 Motion
- Short, meaningful intro motion only:
  - staggered reveal for pill, headline, subcopy, CTA
  - duration range 220ms–420ms
- Must degrade gracefully with reduced-motion preference.

## 7. Component-Level Impact

### 7.1 `index.html`
- Update hero text and CTA labels/targets as specified.
- Optionally add a small tertiary text-link CTA in hero actions.
- Keep IDs/anchors for downstream sections stable.

### 7.2 `home.css`
- Add/adjust CSS variables for top-section visual language.
- Rework Header and Hero styles only; avoid unnecessary global churn.
- Add subtle background/grid treatment.
- Add small motion keyframes and reduced-motion guard.

### 7.3 `home.js`
- Prefer no JS change.
- If tertiary CTA needs analytics, add minimal instrumentation without renaming existing events.

## 8. Data Flow / Behavior

Static homepage behavior remains unchanged:
- Rust server serves embedded HTML/CSS/JS assets via `include_str!` build-time embedding.
- Copy buttons and tabs continue existing behavior.
- Any CTA analytics continue through existing `emitAnalytics` function.

Implication:
- Visual updates require server rebuild/restart in local validation due to build-time embedding.

## 9. Error Handling & Robustness

- Preserve current fallback behavior for copy interactions.
- Ensure no dependence on unavailable fonts/scripts.
- Respect small-screen breakpoints and avoid horizontal overflow in Hero actions.
- Ensure third CTA remains usable on touch devices.

## 10. Accessibility

- Preserve semantic heading hierarchy.
- Maintain sufficient contrast for top nav and hero text.
- Keyboard focus styles remain visible for all CTAs.
- Motion must respect `prefers-reduced-motion`.

## 11. Testing & Validation Plan

Smallest-first validation:
1. `cargo check -p statichub-server`
2. Manual visual check at desktop and mobile widths.
3. Manual interaction checks:
   - hero CTAs navigate correctly
   - existing tabs still switch
   - copy buttons still work
4. Domain/routing sanity in local environment (correct Host header for homepage route).

Optional final confidence check:
- `cargo test -p statichub-server` if structural changes spill over (not expected).

## 12. Risks and Mitigations

Risk: visual overlap with existing Deploy example
- Mitigation: keep hero conceptual; avoid terminal-output duplication in hero.

Risk: CTA hierarchy confusion
- Mitigation: explicit visual priority and clearer label language.

Risk: local verification mismatch due to embedded assets
- Mitigation: always rebuild/restart server before checking rendered result.

## 13. Rollout Strategy

Single PR, scoped to homepage static assets.
No migration required.
If needed, can revert by restoring two files (`index.html`, `home.css`).

## 14. Acceptance Criteria

- Header/Hero visibly upgraded to Editorial AI tone.
- Hero copy matches approved "折中款" direction.
- CTA priority is visually and semantically clear: Skill > Install > Deploy.
- Lower homepage sections remain intact and functional.
- No backend/API behavior changes.
- Basic local checks pass.

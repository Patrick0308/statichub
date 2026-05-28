# Homepage Editorial AI Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade homepage professional brand perception by redesigning Header/Hero in an Editorial AI style while preserving current section structure and behavior.

**Architecture:** Keep existing static homepage pipeline (`index.html` + `home.css` + optional `home.js`) and only adjust top-of-page markup/styles plus minimal analytics-safe CTA instrumentation if needed. Preserve section anchors, tabs, copy interactions, and server routing behavior.

**Tech Stack:** Rust server static embedding, HTML, CSS, vanilla JS.

---

### Task 1: Branch + Baseline Audit

**Files:**
- Modify: none

- [ ] **Step 1: Create feature branch**
Run: `git checkout -b codex/homepage-editorial-ai`
Expected: switched to new branch

- [ ] **Step 2: Verify baseline files**
Run: `rg -n "hero|site-header|top-nav|hero-actions" server/static/home/index.html server/static/home/home.css`
Expected: existing Header/Hero definitions located

### Task 2: Hero Copy + CTA Hierarchy

**Files:**
- Modify: `server/static/home/index.html`

- [ ] **Step 1: Update hero copy to approved direction**
Change pill/headline/subcopy to:
- `For AI publishing workflows`
- `From prompt to production URL.`
- `Use statichub skill for auto-publish workflows, with CLI install and deploy always available.`

- [ ] **Step 2: Update CTA order and labels**
Keep 3-layer priority:
- Primary `Use statichub skill`
- Secondary `Install CLI`
- Tertiary text link `Run first deploy`
Ensure anchors point to existing sections.

### Task 3: Header + Hero Styling Refresh

**Files:**
- Modify: `server/static/home/home.css`

- [ ] **Step 1: Refine header style**
Adjust brand/nav/action visual hierarchy (lighter editorial nav, clear primary button).

- [ ] **Step 2: Refine hero typography and spacing**
Improve display rhythm (headline scale, spacing, readable subcopy width).

- [ ] **Step 3: Add subtle Editorial AI atmosphere**
Use low-contrast gradient/glow/grid-like treatment without reducing readability.

- [ ] **Step 4: Add restrained entry motion + reduced-motion fallback**
Introduce short staggered reveal with `prefers-reduced-motion` guard.

### Task 4: Behavior Preservation and Interaction Safety

**Files:**
- Modify: `server/static/home/home.js` (only if needed)

- [ ] **Step 1: Keep existing tabs/copy behavior unchanged**
Do not alter quickstart/install tab logic.

- [ ] **Step 2: Add analytics hook for new tertiary CTA only if introduced**
Use existing `emitAnalytics` pattern without renaming current events.

### Task 5: Validation

**Files:**
- Modify: none

- [ ] **Step 1: Rust compile check**
Run: `cargo check -p statichub-server`
Expected: success

- [ ] **Step 2: Sanity-check rendered homepage via allowed Host header**
Run: local serve and request homepage with `Host: statichub.dev`; verify updated hero strings.

- [ ] **Step 3: Review diff scope**
Run: `git diff -- server/static/home/index.html server/static/home/home.css server/static/home/home.js`
Expected: focused to planned files.

### Task 6: Commit

**Files:**
- Modify: planned files only

- [ ] **Step 1: Stage intended changes**
Run: `git add ...`

- [ ] **Step 2: Commit with imperative message**
Run: `git commit -m "Refresh homepage hero and header with Editorial AI style"`


# Statichub Server Homepage Design (AI + CLI + Skill)

Date: 2026-05-25
Project: statichub
Scope: Design for `statichub server` homepage at `/`

## 1. Goal
Build a marketing-style homepage on `statichub server` that explains how to install and use `statichub cli`, while clearly positioning Statichub as an AI-native shipping workflow through both CLI and Skill.

Primary outcome: a visitor can quickly understand the AI workflow and complete installation/start steps with minimal friction.

## 2. Audience
- Developers building static sites with AI coding tools.
- Users who may choose either a terminal-first workflow (CLI) or agent-first workflow (Skill).

## 3. Positioning
Core message:
- Statichub is the shipping layer for AI-built static sites.
- Statichub CLI is the terminal path.
- Statichub Skill is the in-agent path.

Key tagline:
- `From AI prompt to production URL.`

Supporting copy:
- `Use Statichub Skill in your AI workflow, or Statichub CLI in your terminal.`

## 4. URL and Routing
Homepage must be mounted at:
- `/`

Existing API/auth routes remain unchanged.

## 5. Page Structure (No Hero, Quickstart-first)
The homepage intentionally does not use a large hero block. It follows an Astro-style, content-first layout.

### 5.1 Top Navigation
- Brand: `statichub`
- Links: `CLI`, `Skill`, `Docs`, `GitHub`
- Right-side CTA: `Install` (scroll to install section)

### 5.2 Intro Strip (Lightweight)
- One concise positioning sentence (tagline + supporting sentence)
- Primary CTA: `Install CLI`
- Secondary CTA: `Use Skill`
- Lightweight trust line: `Built for AI coding workflows.`

### 5.3 Quickstart (First Main Section)
- Two tabs: `Skill-first`, `CLI-first`
- Each tab presents 3 clear steps with copyable command blocks where applicable.

### 5.4 Install Section
- OS segmented control: `macOS`, `Linux`, `Windows`
- For each OS, show: recommended install command + fallback install method
- Metadata links: `Latest version`, `Release notes`

### 5.5 Why Statichub for AI Builders
Three value cards:
- `Speed`
- `Repeatability`
- `Shareability`

Each card includes a concrete user benefit statement.

### 5.6 CLI Capabilities
4-6 concise capability cards with short command examples.

### 5.7 FAQ + Final CTA
- FAQ includes: Skill vs CLI selection guidance, install troubleshooting, upgrade path, auth issues
- Final CTA: `Install now`

## 6. Visual Direction
- Overall style: clean, technical, optimistic.
- Avoid doc-site tone and avoid overly decorative web3-like visuals.
- Typography: expressive display font for headings, readable sans-serif for body, monospace for command blocks
- Color direction: light background + technical accent (electric cyan/neon blue family), avoid default purple-centric look
- Motion: subtle reveal and smooth scroll only, no heavy micro-interaction density

## 7. Interaction Details
- Segmented controls: quickstart path switch (`Skill-first` / `CLI-first`) and OS switch (`macOS` / `Linux` / `Windows`)
- Copy buttons on command blocks with transient feedback (`Copied`).
- Mobile behavior: concise intro copy, horizontal scroll inside code blocks allowed, FAQ collapsed by default

## 8. Error States and Fallbacks
- Copy failure: show `Copy manually` and auto-select command text
- OS detection failure: default to `macOS` and keep manual switching visible
- Skill info unavailable: show fallback guidance to CLI path
- External link issues: keep links visible and open in new tab with non-blocking guidance

## 9. Analytics Events (Lightweight)
Track key conversion interactions:
- `click_install_primary`
- `switch_path_skill`
- `switch_path_cli`
- `copy_install_command` (with OS dimension)
- `click_use_skill`
- `complete_quickstart_step` (step index)

## 10. Definition of Done
- Homepage available at `/`.
- No large hero section; first main area is quickstart-driven.
- User can find and copy install command within 60 seconds.
- Both Skill-first and CLI-first paths are complete and understandable.
- Mobile layout is stable at 390px width (code blocks may scroll internally).
- Performance/SEO/Best-practice quality meets implementation-time baseline targets.

## 11. Out of Scope
- Reworking existing API endpoints.
- Full docs information architecture redesign.
- Multi-page marketing site expansion beyond `/` in this task.

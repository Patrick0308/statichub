# Single Markdown File Deploy Support

**Date:** 2026-06-04
**Status:** Approved for planning

## Overview

StaticHub will support deploying a single Markdown file with `statichub deploy <file.md>`.
The CLI will upload the Markdown source as `index.md`. During deploy processing, the
server will render that Markdown into a complete `index.html` file and store both files
in the deploy. Visitors can access the rendered page at the project root, while the
original Markdown remains available as a stored deploy artifact.

This feature is limited to single-file Markdown deploys. Directory deploy behavior
remains unchanged, including any Markdown files inside directories.

## Goals

- Allow `statichub deploy README.md` and `statichub deploy README.markdown`.
- Keep existing single-file HTML behavior unchanged.
- Render Markdown on the server during deployment.
- Store both the original Markdown file and the generated HTML file.
- Serve the generated HTML through the existing static serving path.
- Avoid database schema changes and keep the deploy API response unchanged.

## Non-Goals

- Rendering Markdown files found inside directory deploys.
- Request-time Markdown rendering.
- Hiding or blocking access to the original `index.md`.
- Adding theme customization, front matter, templates, or per-project Markdown settings.
- Changing clean URL, SPA, redirect, auth, domain, or rollback behavior.

## Current Behavior

`cli/src/upload.rs` supports directory deploys and single-file HTML deploys. A single
`.html` or `.htm` file is uploaded as `index.html`; other single-file extensions are
rejected.

The server stores uploaded multipart files as deploy artifacts. Static serving resolves
`/` to `index.html`, detects content type from the stored path, and returns the stored
bytes. The server does not currently transform uploaded content during deploy
processing.

## Proposed Architecture

### CLI

`collect_files()` will allow single Markdown files in addition to `.html` and `.htm`.
When the input path is a single `.md` or `.markdown` file, the CLI will read the file and
produce one upload:

- `path`: `index.md`
- `content`: original Markdown bytes

The CLI will not render Markdown. This keeps rendering behavior centralized on the
server and avoids different CLI versions producing different HTML.

Single `.html` and `.htm` files will continue to upload as `index.html`.

### Server

The server deploy path will collect validated multipart files before writing the deploy
as ready. If the upload is exactly one file named `index.md`, the server will:

1. Store the original `index.md`.
2. Interpret its bytes as UTF-8 Markdown.
3. Render the Markdown into HTML.
4. Wrap the rendered body in a complete HTML document.
5. Store the generated document as `index.html`.
6. Count both stored files in `file_count` and `total_size_bytes`.

All other deploys will follow the existing storage behavior. In particular, directory
deploys containing Markdown files will store those files as-is and will not generate
HTML.

### Markdown Rendering

The server should use a Rust Markdown renderer such as `pulldown-cmark`. The renderer
should enable a conservative, useful option set:

- Headings, paragraphs, emphasis, links, images, lists, blockquotes, and code blocks.
- Tables and strikethrough if supported cleanly by the chosen renderer.
- Task lists if supported without extra custom parsing.

The generated `index.html` should include:

- `<!doctype html>`
- `<html lang="en">`
- UTF-8 charset meta tag
- viewport meta tag
- title derived from the first Markdown H1, falling back to `StaticHub Markdown`
- minimal embedded CSS for readable document presentation

The HTML shell should be deterministic and small. It should not depend on external
assets.

## Data Flow

1. User runs `statichub deploy README.md`.
2. CLI validates that `README.md` is a supported single-file Markdown path.
3. CLI uploads the file as multipart filename `index.md`.
4. Server sanitizes and reads the multipart file.
5. Server recognizes the single-file Markdown deploy case.
6. Server stores `index.md`.
7. Server renders and stores `index.html`.
8. Server marks the deploy ready and updates the project current deploy pointer.
9. User visits `/` and receives the generated `index.html` through existing static
   serving.

## Error Handling

- Unsupported single-file extensions will fail in the CLI with a message that mentions
  supported `.html`, `.htm`, `.md`, and `.markdown` files.
- Empty Markdown files are allowed. They render to a valid empty HTML document.
- Non-UTF-8 Markdown input will cause the server deploy to fail with a bad request
  error. The deploy record will be marked `failed`.
- Storage errors while saving either `index.md` or `index.html` will fail the deploy and
  mark the deploy `failed`, matching existing behavior.
- Markdown rendering should not panic on malformed Markdown. CommonMark-compatible
  input should render best-effort HTML.

## Security

Markdown rendering must escape raw text according to the renderer's normal HTML escaping
rules. The initial implementation should not intentionally add server-side script
execution, remote asset fetching, or custom HTML post-processing.

Raw HTML handling should follow the chosen renderer's default behavior unless disabling
raw HTML is straightforward and well-supported. If raw HTML is allowed by default, this
must be called out in implementation notes and covered by a focused test so the behavior
is explicit rather than accidental.

Filename sanitization and deploy path validation remain unchanged.

## Compatibility

- Existing directory deploys continue unchanged.
- Existing single `.html` and `.htm` deploys continue unchanged.
- Existing deploy API responses remain unchanged.
- Existing serving behavior remains unchanged after `index.html` has been generated.
- Rollback behavior remains unchanged because both Markdown source and generated HTML
  are ordinary deploy files.

## Testing Plan

### CLI Unit Tests

- Single `.md` file is collected as one upload named `index.md`.
- Single `.markdown` file is collected as one upload named `index.md`.
- Existing `.html` and `.htm` tests still pass.
- Unsupported single-file extensions still fail, with updated supported-extension text.

### Server Tests

- A deploy containing only multipart file `index.md` stores both `index.md` and
  `index.html`.
- Generated `index.html` contains rendered Markdown content.
- Deploy metadata counts both stored files and their total stored byte size.
- Non-UTF-8 `index.md` fails the deploy and marks the deploy failed.
- A deploy containing `index.md` plus another file does not trigger special single-file
  Markdown rendering.

### Serving Tests

- After a single Markdown deploy, requesting `/` returns `text/html` content from
  generated `index.html`.
- Requesting `/index.md` returns the original Markdown with the existing MIME detection
  behavior.

## Implementation Notes

The current `process_multipart_files()` streams each multipart field directly to
storage. Supporting the special case cleanly will likely require refactoring it to first
collect validated upload fields into a small internal struct, enforce the existing file
size, total size, and file count limits, then store either the original set or the
Markdown-expanded set.

The refactor should stay local to deploy processing and avoid changing the storage trait,
database schema, or response types.

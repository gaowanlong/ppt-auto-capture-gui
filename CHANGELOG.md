# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Newest releases come first. **Every release tag needs a section here**: the
release workflow publishes the matching section as the GitHub release body and
refuses to release a tag that has none, so the notes on the releases page always
say what actually changed.

## [Unreleased]

### Changed

- Release notes now come from this file. The release workflow publishes the
  section for the tag as the GitHub release body and stops at preflight when the
  section is missing, instead of shipping generated notes that only link a
  comparison because commits go straight to `main`.

## [2.8.3] - 2026-11-02

### Fixed

- Choosing a folder or a `.pptx` on Windows no longer flashes a black console
  window over the desktop. The application is a GUI binary and owns no console,
  so Windows allocated one for the PowerShell helper behind each chooser; the
  helpers now start with `CREATE_NO_WINDOW`. The dialog is unchanged, only its
  console is gone.
- The chooser result now travels as the hex of its UTF-8 bytes instead of plain
  text, so a path such as `C:\资料\幻灯片.pptx` arrives intact. This also drops
  the `[Console]::OutputEncoding` call, which a helper without a console cannot
  rely on.

## [2.8.2] - 2026-11-02

### Fixed

- The Create PPTX mode switch keeps its height in both states. The selected
  option was drawn with the primary button style and stood 2px taller in a
  larger font, so the whole row jumped every time the other mode was picked.

## [2.8.1] - 2026-11-02

### Added

- A regression test proving that a deck this tool already trimmed is not trimmed
  a second time: every media part comes back byte for byte identical, so a
  repeated pass cannot eat into slide content.

### Changed

- The Create PPTX page title, subtitle, and Chinese sidebar label now name both
  modes instead of describing the image folder only.

### Fixed

- A picture carrying `<a:srcRect l="0" t="0" r="0" b="0"/>`, which PowerPoint
  writes on some round trips, is no longer treated as already cropped and is
  trimmed like any other.

## [2.8.0] - 2026-11-02

### Added

- **Trim a deck that already exists.** The Create PPTX page has a second mode:
  point it at a finished `.pptx` and it writes a new deck whose embedded slide
  screenshots have their window chrome, taskbar, meeting controls, and letterbox
  bars trimmed. The source file is only read.
  - The package is parsed through `presentation.xml`, slide relationships, and
    each `p:pic` offset and extent. Only the trimmed media parts and those
    pictures' geometry are rewritten; page size, slide order, notes, and every
    other part are copied byte for byte.
  - Trimming reuses the existing detector with a cross-slide frame reference, so
    a taskbar or control strip that is identical on every slide is removed while
    content that changes between slides is kept. Only pictures covering at least
    a third of a slide count as screenshots, and pictures that already carry a
    crop rectangle or are rotated or flipped are skipped, so logos and
    decorations stay untouched.
  - Two placements: **fill the original area** (a full-bleed picture grows to the
    page, removing the margin the bars used to occupy) and **keep the original
    scale** (edges only, no resampling).
  - A crop that matches the frame within half a percent is snapped to it, so a
    full-page screenshot cannot leave a hairline of background around itself.
  - Trimmed pixels are staged on disk before any slide is edited, so a picture
    that cannot be re-encoded keeps both its bytes and its geometry. The new
    export folder is removed if the deck fails or cannot be verified.
- A `.pptx` file chooser for each platform, the placement choice, progress
  reporting, and a localized result line on the Create PPTX page.

### Fixed

- The Linux `.pptx` chooser honours its starting folder. The ignored argument
  also pushed the Clippy warning count over its ceiling on Linux builds.

## Earlier releases

Releases before 2.8.0 predate this file. Their notes live in the commit history
and in the tag comparison links on the releases page.

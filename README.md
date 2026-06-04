# difft-dir-viewer

[English](README.md) | [简体中文](README.zh-CN.md)

Slint GUI for comparing **two directories** side by side.

**Use this viewer together with [difft-file-viewer](../difft-file-viewer/)** — it depends on the **patched `difft`** built from `difft-file-viewer/difftastic/` (version **0.70.0**). Do **not** use an unpatched upstream `difft` from Homebrew/winget for the GUI JSON format.

The viewer spawns `difft`, reads `--display json` output, shows a **Changed files** sidebar, and renders the selected file in side-by-side code panes (same fonts, colours, and keyboard bindings as difft-file-viewer).

## How it works

```
┌─────────────────┐     subprocess      ┌──────────────────────────┐
│ difft-dir-      │  DFT_UNSTABLE=yes   │  difft (patched          │
│ viewer (Slint)  │ ──────────────────► │  difft-file-viewer/      │
└─────────────────┘  --display json     │  difftastic)             │
        │                               └──────────────────────────┘
        │  parse JSON array + read each changed file from disk
        ▼
   Slint UI — Changed files | A | B
```

1. Pass **directory A** and **directory B** on the command line (**required** — exactly two paths).
2. The viewer runs:

   ```text
   difft --display json --skip-unchanged --byte-limit 32000000 --context 999999 <dirA> <dirB>
   ```

   (With `DFT_UNSTABLE=yes` and `DFT_PARSE_ERROR_LIMIT=4096` set in the subprocess environment.)

3. Parse the JSON **array** — one entry per changed file (`--skip-unchanged` omits identical files from the list). Line text is read from disk with **UTF-8 lossy** decoding (invalid bytes become U+FFFD replacement characters); JSON carries alignment and change metadata (same format as difft-file-viewer). Per-file read/parse failures are skipped when possible; a summary may appear in the info overlay.
4. **Changed files** list shows relative paths with a status tag (binary files are omitted; use `-e` to restrict by extension):
   - **M** — modified (both sides)
   - **A** — added (only in B)
   - **D** — deleted (only in A)
5. Selecting a file updates the side-by-side code panes.

On success, status messages are hidden. Errors and diff fallback messages appear in a **bottom overlay** on the code pane (purple info text; same role as difft-file-viewer’s info area, but does not shrink the main layout).

### Nested directories

`difft` walks subdirectories recursively. The `path` field in JSON is the **relative path from the chosen root**, for example:

```text
src/main.rs
sub/deep/f.rs
.hidden/doc.txt
```

The sidebar displays that full relative path (240px column; long paths scroll horizontally). There is **no directory tree** — only a flat list as returned by `difft`.

### Added / deleted files

For **A** and **D** entries, `difft` may return empty `aligned_lines`. The viewer reads the file from disk under directory A or B and fills one side of the panes.

## Requirements

| Component | Version / notes |
|-----------|-----------------|
| Rust | 1.85+ (see `rust-version` in `Cargo.toml`) |
| Slint | **1.16** (`backend-winit` + `renderer-femtovg`) |
| `difft` | **Build from `difft-file-viewer/difftastic/`** (0.70.0 + GUI patches) |
| OS | macOS, Linux, Windows (Slint + winit) |

The viewer sets `DFT_UNSTABLE=yes` because JSON output is still an **unstable** difftastic feature.

## Platform support

| Area | Notes |
|------|--------|
| GUI | Slint `backend-winit` + `renderer-femtovg` |
| Window | Borderless (`no-frame`); macOS **native fullscreen**, Linux maximized, Windows **work area** |
| macOS | Dock / Cmd+Tab icon via `macos_icon`; `macos_edge::fill_screen` (`set_fullscreen`) on launch |
| Windows | `windows_edge` (`#[cfg(target_os = "windows")]`) — borderless work-area sizing, no extra console in Release |
| Windows `difft` | `difft.exe`, `where difft`, optional `.exe` on `DIFT_PATH` / `--difft` |
| Subprocess | `CREATE_NO_WINDOW` so `difft` does not flash a console (Windows) |
| Install hints | `winget`, `scoop`, `choco` in `install_message()` when `difft` is missing |

Windows-only code (`windows_edge.rs`, Win32 deps) is gated with `#[cfg(target_os = "windows")]` so **macOS/Linux builds are unchanged**.

## Building (with difft-file-viewer)

Typical layout in a shared workspace:

```text
slint-viewer/
├── difft-file-viewer/
│   └── difftastic/          ← build difft here first
└── difft-dir-viewer/        ← this crate
```

From the **`slint-viewer/`** root:

```bash
cargo build --manifest-path difft-file-viewer/difftastic/Cargo.toml
cargo build --manifest-path difft-dir-viewer/Cargo.toml
cargo test --manifest-path difft-dir-viewer/Cargo.toml
```

Binaries (debug):

- `difft-file-viewer/difftastic/target/debug/difft` (`.exe` on Windows)
- `difft-dir-viewer/target/debug/difft-dir-viewer` (`.exe` on Windows)

Release:

```bash
cargo build --release --manifest-path difft-file-viewer/difftastic/Cargo.toml
cargo build --release --manifest-path difft-dir-viewer/Cargo.toml
```

Windows (PowerShell, from `slint-viewer/`):

```powershell
cargo build --manifest-path difft-file-viewer/difftastic/Cargo.toml
cargo run --manifest-path difft-dir-viewer/Cargo.toml -- dir-a dir-b
```

## Installing / locating `difft`

The viewer resolves `difft` in this order:

1. `--difft PATH` on the command line
2. `DIFT_PATH` environment variable
3. `difftastic/target/debug|release/difft` or `difft-file-viewer/difftastic/target/debug|release/difft` relative to the current working directory (or its parent)
4. Same paths relative to the viewer executable (up to a few parent directories)
5. `difft` next to the viewer executable
6. `difft` on `PATH` (`which` / `where`)

If `difft` is missing or is an **unpatched** upstream build, JSON parsing may fail. The UI shows install hints when no working binary is found.

Verify:

```bash
difft --version
```

## Usage

```bash
difft-dir-viewer [--difft PATH] [-e EXT]... <dir-a> <dir-b>
```

Examples (from `slint-viewer/`):

```bash
cargo run --manifest-path difft-dir-viewer/Cargo.toml -- \
  difft-file-viewer/difftastic/sample_files/dir_1 \
  difft-file-viewer/difftastic/sample_files/dir_2

cargo run --manifest-path difft-dir-viewer/Cargo.toml -- \
  -e cpp -e h dir-a dir-b
```

**Exactly two** directory paths are required. Diff starts automatically on launch. The top bar shows the **same path strings you passed on the command line** (not canonicalized `\\?\` / UNC forms).

### File extension filter (`-e`)

| Option | Behaviour |
|--------|-----------|
| *(none)* | Compare **text files only** (binary files are omitted from the list) |
| `-e EXT` (repeatable) | Compare **text files** whose suffix matches one of the given extensions (`cpp`, `.cpp`, and `CPP` are equivalent) |

Extensionless paths (e.g. `Makefile`) are included when no `-e` is given, and excluded when `-e` is used.

## UI layout

Shared with difft-file-viewer where applicable (fonts, colours, shortcuts):

- **Borderless window** — no system title bar; quit with **`q`** (or close from the taskbar / **`Alt+F4`** on Windows).
- **Top row (one line, full width):** **Changed files (N)** on the left (240px column) and directory **A / B path labels** on the right, same height.
- **Main area:** changed-files list (left) + side-by-side code panes (right), filling the rest of the window edge-to-edge. Panel backgrounds reach the window edges; text uses a small **4px gutter** so characters are not clipped.
- **Info overlay:** when present, status / warning text floats over the **bottom** of the code pane (~72px); hidden on a clean successful diff so the list and code use the full height.

Code panes use **Courier New**, bold weight, Dracula-style colours, compact line height (`font-size + 4px`), fixed line-number gutter, and horizontal scroll for long lines.

## Keyboard shortcuts

Focus must be on the diff panel for code scrolling (auto-focused after diff completes or when you click the code pane). On macOS, **Ctrl** below also matches **⌘ (Meta)** for the same actions.

### Scrolling (code pane focused)

| Key | Action |
|-----|--------|
| `Page Up` / `Page Down` | Scroll one page |
| `Ctrl+b` / `Ctrl+f` | Scroll one page |
| `Ctrl+u` / `Ctrl+d` | Half page |
| `Home` / `End`, `G`, `g` `g` | Top / bottom |
| `h` / `l` | Scroll code horizontally (gutter fixed) |
| Trackpad / wheel | Smooth vertical scroll; horizontal when the gesture is mostly horizontal |

### Scrolling (Changed files sidebar focused)

Click the sidebar first — the same scroll keys (`h` / `l`, page keys, `Home` / `End`, `G`, `g` `g`) apply to the **file list** instead of the code pane. Path text scrolls horizontally; the status column stays fixed. Wheel/trackpad scrolling in the sidebar is also smooth/damped.

### Font size

| Key | Action |
|-----|--------|
| `Ctrl+=` / `Ctrl++` | Increase code font (8–24 px) |
| `Ctrl+-` | Decrease code font |

Line height, gutter width, and horizontal scroll step scale with font size. Font-size shortcuts work regardless of sidebar vs code focus.

**Scroll feel:** wheel and trackpad use velocity decay (~60 fps); **keyboard** (`Page Up`/`Down`, `Home`/`End`, `h`/`l`, etc.) jumps instantly with no animation.

### Other

| Key | Action |
|-----|--------|
| `Escape` | Leave sidebar focus; return shortcuts to the code pane |
| `q` | Quit the viewer |
| `Alt+F4` | Quit (Windows, standard close shortcut) |

## JSON format (important)

Directory mode returns a **JSON array** of objects. Each object uses the same **patched** `--display json` shape as difft-file-viewer:

| Field | Purpose |
|-------|---------|
| `path`, `language`, `status` | File metadata (`changed` / `created` / `deleted` / `unchanged`) |
| `extra_info` | Optional human-readable note |
| `aligned_lines` | `[[lhs_index, rhs_index], …]` — 0-based line indices |
| `chunks` | Per-line change metadata (spans, highlights) keyed by alignment |

The viewer reads **line text from disk** under directory A / B. Legacy JSON with embedded `lhs_text` / `rhs_text` per aligned row is still accepted if present.

- Requires `DFT_UNSTABLE=yes` (set automatically).
- **No stability guarantee** — field names may change between releases.
- **Always build `difft` from `difft-file-viewer/difftastic/`** and keep both viewers on the same revision.

## Behaviour notes

| Topic | Detail |
|-------|--------|
| Companion | **Requires patched `difft` from difft-file-viewer** — not a standalone difftastic install |
| Path labels | Top bar shows **CLI path strings**; disk I/O uses resolved absolute paths |
| Inputs | **Directories only** — a file path shows an error |
| Multi-level trees | Supported via `difft` recursion; list shows `dir/file` paths |
| Flat list | No folder grouping or collapse |
| File size | `--byte-limit 32000000` (32 MiB) per file |
| Parse errors | `DFT_PARSE_ERROR_LIMIT=4096` in the `difft` subprocess |
| Context | `--context 999999` (essentially full file in the GUI) |
| Encoding | Files read as bytes, decoded with **UTF-8 lossy**; GBK/mixed sources may show U+FFFD or mojibake but rarely abort the whole directory diff |
| Line numbers | Display 1-based; JSON indices 0-based |
| Tabs | Display expands tabs for alignment (Courier New) |
| Long lines | Horizontal scroll via `h` / `l` |
| Ignore rules | Same as CLI (`.gitignore`, etc.) — not controlled by the viewer |

## Troubleshooting

| Symptom | Likely cause |
|---------|----------------|
| `difft not found` | Build `difft-file-viewer/difftastic/` or set `--difft` / `DIFT_PATH` |
| Purple JSON parse error | Unpatched or mismatched `difft` — rebuild from `difft-file-viewer/difftastic/` |
| `Path A is not a directory` | Pick a directory, not a single file |
| Empty changed list | No differences, or all files unchanged |
| U+FFFD or garbled Chinese in code | Non–UTF-8 source (e.g. GBK); viewer uses lossy UTF-8 — diff still runs |
| Skipped N file(s)… in overlay | Some files failed read/parse; others still listed |
| A/D file empty in panes | Missing file on disk or alignment empty — check purple overlay message |

## License

MIT — same as difftastic.

## Related projects

- **[difft-file-viewer](../difft-file-viewer/)** — file and triple-pane diff viewer; **build `difft` from this repo first**.
- **[difftastic](https://github.com/wilfred/difftastic)** — upstream structural diff engine by Wilfred Hughes.
- **[Slint](https://slint.dev/)** — UI toolkit used by both viewers.

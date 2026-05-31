# difft-dir-viewer

[English](README.md) | [简体中文](README.zh-CN.md)

Slint GUI for comparing **two directories** with [difftastic](https://github.com/wilfred/difftastic).

The viewer spawns the `difft` CLI, reads `--display json` output, and shows a **Changed files** sidebar plus side-by-side code panes.

## How it works

```
┌─────────────────┐     subprocess      ┌──────────┐
│ difft-dir-      │  DFT_UNSTABLE=yes   │  difft   │
│ viewer (Slint)  │ ──────────────────► │  (CLI)   │
└─────────────────┘  --display json     └──────────┘
        │                                      │
        │    stdout: JSON array of objects     │
        └──────────────────────────────────────┘
```

1. Pass **directory A** and **directory B** on the command line (**required** — exactly two arguments).
2. The viewer runs:

   ```text
   difft --display json --byte-limit 32000000 --context 3 <dirA> <dirB>
   ```

3. JSON **array** is parsed — one entry per changed file (unchanged files omitted by default).
4. **Changed files** list shows relative paths with a status tag:
   - **M** — modified (both sides)
   - **A** — added (only in B)
   - **D** — deleted (only in A)
5. Selecting a file updates the side-by-side code panes.

### Nested directories

`difft` walks subdirectories recursively. The `path` field in JSON is the **relative path from the chosen root**, for example:

```text
src/main.rs
sub/deep/f.rs
.hidden/doc.txt
```

The sidebar displays that full relative path (240px column; long paths scroll horizontally). There is **no directory tree** — only a flat list as returned by `difft`.

### Added / deleted files

For **A** and **D** entries, `difft` often returns empty `aligned_lines`. The viewer reads the file from disk under directory A or B and fills one side of the panes.

## Requirements

| Component | Version / notes |
|-----------|-----------------|
| Rust | 1.85+ (see `rust-version` in `Cargo.toml`) |
| `difft` | Same release as this crate (currently **0.70.0**) recommended |
| OS | macOS, Linux, Windows (Slint + winit) |

The viewer sets `DFT_UNSTABLE=yes` on the subprocess (JSON output is unstable upstream).

## Platform support

The crate is **cross-platform** (macOS, Linux, Windows). There is no Unix-only GUI or I/O path in the viewer itself.

| Area | Windows handling (in source) |
|------|------------------------------|
| GUI | Slint `backend-winit` + `renderer-femtovg` (Windows supported) |
| Release binary | `windows_subsystem = "windows"` — no extra console window on launch |
| `difft` lookup | `difft.exe` name, `where difft`, optional `.exe` suffix on `DIFT_PATH` |
| Subprocess | `CREATE_NO_WINDOW` so `difft` does not flash a console (`difft_probe.rs`) |
| Paths / CLI | `std::path` + `args_os()` — no hard-coded `/` separators |
| Install hints | Windows-specific message (`winget`, `scoop`, `choco`) in `install_message()` |

This repository is **standalone**. Install or build `difft` separately (see below); the viewer invokes it as a subprocess.

## Building

Clone this repository, then:

```bash
cargo build
cargo test
```

Binaries:

- `target/debug/difft-dir-viewer` (`.exe` on Windows)

Windows (PowerShell or cmd):

```powershell
cargo build
set DIFT_PATH=C:\path\to\difft.exe
target\debug\difft-dir-viewer.exe dir-a dir-b
```

Release:

```bash
cargo build --release
```

## Installing `difft`

The viewer looks for `difft` in this order:

1. `DIFT_PATH` environment variable
2. `difft` / `difft.exe` next to the viewer executable
3. `difft` on `PATH` (`which` / `where`)

If `difft` is missing, the UI shows install hints.

**macOS**

```bash
brew install difftastic
# or build from https://github.com/wilfred/difftastic and set:
export DIFT_PATH="/path/to/difft"
```

**Windows**

```powershell
winget install Wilfred.difftastic
# or build difftastic from source and set:
set DIFT_PATH=C:\path\to\difft.exe
```

**Linux** — use your package manager, or build from source and set `DIFT_PATH`.

Verify with `difft --version`.

## Usage

```bash
cargo run -- /path/to/dir-a /path/to/dir-b
```

**Exactly two** directory paths are required. Diff starts automatically on launch; the top bar shows paths only (display-only, no folder picker).

Release binary:

```bash
difft-dir-viewer /path/to/dir-a /path/to/dir-b
```

## UI layout (brief)

- Top row: two directory paths (read-only labels).
- Purple area: errors and warnings only (hidden on a clean successful diff).
- Left: **Changed files (N)** in a dark panel.
- Right: side-by-side code lines.

## Keyboard shortcuts

Focus must be on the diff code panel (it receives focus after you switch files). Shortcuts follow common Vim-style bindings.

On macOS, **Ctrl** in the table below also matches **⌘ (Meta)** for the same actions.

### Scrolling

| Key | Action |
|-----|--------|
| `Page Up` | Scroll up one page |
| `Page Down` | Scroll down one page |
| `Ctrl+b` | Scroll up one page |
| `Ctrl+f` | Scroll down one page |
| `Ctrl+u` | Scroll up half a page |
| `Ctrl+d` | Scroll down half a page |
| `Home` | Scroll to top |
| `End` | Scroll to bottom |
| `G` or `Shift+g` | Scroll to bottom |
| `g` then `g` | Scroll to top (press `g` twice) |
| `h` | Scroll code **left** (long lines; gutter stays fixed) |
| `l` | Scroll code **right** |
| Trackpad / wheel (code pane) | Horizontal scroll when the gesture is mostly horizontal |

Line numbers sit in a fixed **gutter** column; only the code pane scrolls horizontally.

**Changed files** sidebar: paths scroll horizontally (status column stays fixed). Click the sidebar to route the shortcuts above to the file list; click the code pane to route them to the diff view.

## JSON format (important)

Directory mode returns a **JSON array** of objects (not a single object). Each object includes:

- `path`, `language`, `status` (`changed` / `created` / `deleted` / `unchanged`)
- `extra_info` (optional)
- `aligned_lines[]` with `lhs_text`, `rhs_text`, `is_novel_lhs`, `is_novel_rhs`, plus optional span metadata

Upstream explicitly states that `--display json` is **experimental**:

- Requires `DFT_UNSTABLE=yes`.
- Format **may change** without a version field in the output.
- Not a semver-stable public API.

The viewer:

- Parses the full array (not just the first file).
- Ignores unknown JSON fields where possible.
- Breaks if required fields disappear or are renamed.

**Pin `difft` and the viewer to the same git revision** when deploying.

## Behaviour notes

| Topic | Detail |
|-------|--------|
| Inputs | **Directories only** — selecting a file shows an error |
| Multi-level trees | Supported via `difft` recursion; list shows `dir/file` paths |
| Flat list | No folder grouping or collapse |
| Large trees | Many changed files → long sidebar; performance depends on `difft` |
| Ignore rules | Same as CLI (`.gitignore`, etc.) — not controlled by the viewer |
| Byte limit | 32 MiB per file (`--byte-limit 32000000`) |
| Long lines | Horizontal scroll in the code pane (`h` / `l` or trackpad); gutter stays fixed |
| Long paths in Changed files | Horizontal path scroll; click sidebar first so `h`/`l`/page keys scroll the file list |

## Troubleshooting

| Symptom | Likely cause |
|---------|----------------|
| `Path A is not a directory` | Pick a directory, not a single file |
| Empty changed list | No differences, or all files unchanged |
| A/D file empty in panes | Read error — check purple message; binary files may fail `read_to_string` |
| `difft not found` | Set `DIFT_PATH` or install `difftastic` |
| JSON parse error | Mismatched `difft` version — rebuild from same repo |

## License

MIT — same as difftastic.

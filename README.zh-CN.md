# difft-dir-viewer

[English](README.md) | [简体中文](README.zh-CN.md)

基于 Slint 的 GUI，用于并排对比 **两个目录**。

**请与 [difft-file-viewer](../difft-file-viewer/) 配套使用** — 本 viewer 依赖 `difft-file-viewer/difftastic/` 中构建的 **patched `difft`**（版本 **0.70.0**）。请勿用 Homebrew/winget 等渠道的上游 `difft` 替代（GUI JSON 格式不兼容）。

Viewer 子进程调用 `difft`，读取 `--display json` 输出，左侧展示 **Changed files** 列表，右侧双栏显示当前文件 diff（字体、配色、快捷键与 difft-file-viewer 一致）。

## 基本机制

```
┌─────────────────┐     subprocess      ┌──────────────────────────┐
│ difft-dir-      │  DFT_UNSTABLE=yes   │  difft（patched          │
│ viewer (Slint)  │ ──────────────────► │  difft-file-viewer/      │
└─────────────────┘  --display json     │  difftastic）            │
        │                               └──────────────────────────┘
        │  解析 JSON 数组 + 从磁盘读取各变更文件
        ▼
   Slint UI — Changed files | A | B
```

1. 启动时传入目录 A、目录 B（**必须**两个命令行参数）。
2. Viewer 执行：

   ```text
   difft --display json --byte-limit 32000000 --context 999999 <dirA> <dirB>
   ```

   （子进程环境中会设置 `DFT_UNSTABLE=yes` 与 `DFT_PARSE_ERROR_LIMIT=4096`。）

3. 解析 JSON **数组** — 每个变更文件一项（默认不含 unchanged）。行文本从磁盘读取，采用 **UTF-8 lossy** 解码（非法字节显示为 U+FFFD 替换字符）；JSON 携带对齐与变更元数据（与 difft-file-viewer 相同格式）。单文件读盘/解析失败时尽量跳过其余文件；汇总信息可能出现在底部 overlay。
4. **Changed files** 列表显示相对路径与状态标记：
   - **M** — 修改（两侧都有）
   - **A** — 新增（仅在 B）
   - **D** — 删除（仅在 A）
5. 点击文件名切换右侧 diff 内容。

成功时隐藏状态行；错误与 fallback 警告显示在代码区 **底部 overlay**（紫色信息文字；作用与 difft-file-viewer 的信息区相同，但不占用主布局高度）。

### 多层目录

`difft` 递归遍历子目录。JSON 中 `path` 为 **相对所选根目录的路径**，例如：

```text
src/main.rs
sub/deep/f.rs
.hidden/doc.txt
```

侧栏显示完整相对路径（列宽 240px，过长可横向滚动）。**无目录树**，仅为 `difft` 返回顺序的扁平列表。

### 新增 / 删除文件

**A** / **D** 条目可能返回空的 `aligned_lines`。Viewer 会从目录 A 或 B 读盘，填充单侧代码栏。

## 环境要求

| 组件 | 说明 |
|------|------|
| Rust | 1.85+（见 `Cargo.toml` 中 `rust-version`） |
| Slint | **1.16**（`backend-winit` + `renderer-femtovg`） |
| `difft` | **从 `difft-file-viewer/difftastic/` 构建**（0.70.0 + GUI patches） |
| 系统 | macOS、Linux、Windows（Slint + winit） |

Viewer 会在子进程中设置 `DFT_UNSTABLE=yes`（JSON 输出仍为实验性功能）。

## 平台支持

| 方面 | 说明 |
|------|------|
| GUI | Slint `backend-winit` + `renderer-femtovg` |
| 窗口 | 无边框（`no-frame`）；启动时 **最大化**（macOS/Linux）或对齐显示器 **工作区**（Windows） |
| macOS | `macos_icon` 设置 Dock / Cmd+Tab 图标；启动 `set_maximized(true)` |
| Windows | `windows_edge`（`#[cfg(target_os = "windows")]`）— 无边框工作区贴边；Release 无额外控制台 |
| 查找 `difft` | `difft.exe`、`where difft`、`DIFT_PATH` / `--difft` 可带 `.exe` 后缀 |
| 子进程 | Windows 下 `CREATE_NO_WINDOW`，避免弹出控制台 |
| 安装提示 | 未找到 `difft` 时 `install_message()` 含 `winget`、`scoop`、`choco` 说明 |

仅 Windows 相关代码（`windows_edge.rs`、Win32 依赖）使用 `#[cfg(target_os = "windows")]` 隔离，**不影响 macOS/Linux 构建**。

## 构建（与 difft-file-viewer 配套）

典型 monorepo 布局：

```text
slint-viewer/
├── difft-file-viewer/
│   └── difftastic/          ← 先在这里构建 difft
└── difft-dir-viewer/        ← 本 crate
```

在 **`slint-viewer/`** 根目录：

```bash
cargo build --manifest-path difft-file-viewer/difftastic/Cargo.toml
cargo build --manifest-path difft-dir-viewer/Cargo.toml
cargo test --manifest-path difft-dir-viewer/Cargo.toml
```

产物（debug）：

- `difft-file-viewer/difftastic/target/debug/difft`（Windows 为 `.exe`）
- `difft-dir-viewer/target/debug/difft-dir-viewer`（Windows 为 `.exe`）

Release：

```bash
cargo build --release --manifest-path difft-file-viewer/difftastic/Cargo.toml
cargo build --release --manifest-path difft-dir-viewer/Cargo.toml
```

Windows（PowerShell，在 `slint-viewer/` 下）：

```powershell
cargo build --manifest-path difft-file-viewer/difftastic/Cargo.toml
cargo run --manifest-path difft-dir-viewer/Cargo.toml -- dir-a dir-b
```

## 安装 / 定位 `difft`

查找顺序：

1. 命令行 `--difft PATH`
2. 环境变量 `DIFT_PATH`
3. 相对当前工作目录（或其上一级）的 `difftastic/target/...` 或 `difft-file-viewer/difftastic/target/...`
4. 相对 viewer 可执行文件向上若干层的相同路径
5. 与 viewer 同目录下的 `difft` / `difft.exe`
6. `PATH` 中的 `difft`

未找到或使用 **未 patch** 的上游 `difft` 时，JSON 解析可能失败；界面会显示安装提示。

验证：`difft --version`

## 使用

```bash
difft-dir-viewer [--difft PATH] <dir-a> <dir-b>
```

示例（在 `slint-viewer/` 下）：

```bash
cargo run --manifest-path difft-dir-viewer/Cargo.toml -- \
  difft-file-viewer/difftastic/sample_files/dir_1 \
  difft-file-viewer/difftastic/sample_files/dir_2
```

**必须**提供两个目录路径；启动后自动 diff。顶栏显示 **命令行传入的路径字符串**（非 canonicalize 后的 `\\?\` / UNC 形式）。

## 界面布局

与 difft-file-viewer 一致的部分（字体、配色、快捷键）：

- **无边框窗口** — 无系统标题栏；用 **`q`** 退出（Windows 亦可用任务栏关闭 / **`Alt+F4`**）。
- **顶栏（单行、全宽）：** 左侧 **Changed files (N)**（240px 列），右侧目录 **A / B 路径标签**，同一行对齐。
- **主区域：** 变更文件列表（左）+ 双栏 diff（右），占满剩余空间、面板背景贴窗口边缘；文字保留 **4px gutter**，避免字符被裁切。
- **信息 overlay：** 有警告/错误时在代码区 **底部** 浮层显示（约 72px 高，紫色文字）；成功且无警告时隐藏，列表与代码区占满高度。

代码区使用 **Courier New** 粗体、Dracula 配色、紧凑行高（`字号 + 4px`）、固定行号 gutter、长行可横向滚动。

## 快捷键

代码区滚动需焦点在 diff 面板（diff 完成后自动聚焦，或点击代码区）。macOS 上 **Ctrl** 对相同操作也匹配 **⌘ (Meta)**。

### 滚动（焦点在代码区）

| 按键 | 作用 |
|------|------|
| `Page Up` / `Page Down` | 翻一整页 |
| `Ctrl+b` / `Ctrl+f` | 翻一整页 |
| `Ctrl+u` / `Ctrl+d` | 翻半页 |
| `Home` / `End`、`G`、`g` `g` | 顶部 / 底部 |
| `h` / `l` | 代码区水平滚动（gutter 固定） |
| 触摸板 / 滚轮 | 纵向平滑滚动；以水平滑动为主时走横向 |

### 滚动（焦点在 Changed files 侧栏）

先点击侧栏 — 相同滚动键（`h`/`l`、翻页、`Home`/`End`、`G`、`g` `g`）作用于 **文件列表**。路径可横向滚动，状态列固定。侧栏滚轮/触控板同样为阻尼平滑滚动。

### 字号

| 按键 | 作用 |
|------|------|
| `Ctrl+=` / `Ctrl++` | 增大代码字号（8–24 px） |
| `Ctrl+-` | 减小代码字号 |

行高、gutter 宽度、水平滚动步长随字号缩放。字号快捷键在侧栏或代码区聚焦时均可用。

**滚动手感：** 滚轮/触控板带速度衰减（约 60fps）；**键盘**（`Page Up`/`Down`、`Home`/`End`、`h`/`l` 等）仍为瞬时跳转，无动画。

### 其他

| 按键 | 作用 |
|------|------|
| `Escape` | 退出侧栏焦点，快捷键回到代码区 |
| `q` | 退出 viewer |
| `Alt+F4` | 退出（Windows 标准关闭） |

## JSON 格式（重要）

目录 diff 的 stdout 为 **JSON 数组**。每项与 difft-file-viewer 使用相同的 **patched** `--display json` 结构：

| 字段 | 含义 |
|------|------|
| `path`、`language`、`status` | 文件元数据 |
| `extra_info` | 可选说明 |
| `aligned_lines` | `[[lhs_index, rhs_index], …]` — 0-based 行索引 |
| `chunks` | 按对齐行索引的变更 span / 高亮 |

Viewer 从目录 A/B 下 **读磁盘行文本**。仍兼容旧版 embedded `lhs_text` / `rhs_text` 格式。

- 需要 `DFT_UNSTABLE=yes`（自动设置）。
- **无稳定保证** — 字段可能随版本变更。
- **请始终从 `difft-file-viewer/difftastic/` 构建 `difft`**，并与两个 viewer 保持同一 revision。

## 行为说明

| 项目 | 说明 |
|------|------|
| 配套 | **依赖 difft-file-viewer 的 patched difft**，非独立 difftastic 安装 |
| 路径标签 | 顶栏显示 **CLI 原始路径**；读盘使用解析后的绝对路径 |
| 输入 | 仅 **目录** — 传入文件路径会报错 |
| 多层目录 | 由 `difft` 递归；列表显示 `dir/file` 形式路径 |
| 列表 | 扁平列表，无分组/折叠 |
| 文件大小 | 单文件 `--byte-limit 32000000`（32 MiB） |
| 解析错误上限 | 子进程设置 `DFT_PARSE_ERROR_LIMIT=4096` |
| Context | `--context 999999`（GUI 中 essentially 全文件） |
| 编码 | 按字节读盘，**UTF-8 lossy** 解码；GBK 等混合源可能出现 U+FFFD 或乱码，但很少导致整目录 diff 失败 |
| 行号 | 显示 1-based；JSON 索引 0-based |
| Tab | 显示时展开 tab（Courier New 对齐） |
| 长行 | `h` / `l` 或触摸板横向滚动 |
| 忽略规则 | 与 CLI 相同（如 `.gitignore`），viewer 不可配置 |

## 故障排查

| 现象 | 可能原因 |
|------|----------|
| `difft not found` | 构建 `difft-file-viewer/difftastic/` 或设置 `--difft` / `DIFT_PATH` |
| 紫色 JSON 解析错误 | 未 patch 或版本不匹配 — 从 `difft-file-viewer/difftastic/` 重建 |
| `Path A is not a directory` | 请选择目录，不要选单个文件 |
| Changed files 为空 | 无差异或全部 unchanged |
| 代码中出现 U+FFFD 或中文乱码 | 非 UTF-8 源文件（如 GBK）；viewer 使用 lossy UTF-8 — diff 仍可进行 |
| overlay 显示 Skipped N file(s)… | 部分文件读盘/解析失败；其余文件仍可见 |
| A/D 文件内容区空白 | 磁盘上无对应文件或对齐为空 — 查看紫色 overlay 提示 |

## 许可证

MIT — 与 difftastic 相同。

## 相关项目

- **[difft-file-viewer](../difft-file-viewer/)** — 文件 / 三栏 diff viewer；**请先在此仓库构建 `difft`**。
- **[difftastic](https://github.com/wilfred/difftastic)** — 上游 structural diff 引擎（Wilfred Hughes）。
- **[Slint](https://slint.dev/)** — 两个 viewer 使用的 UI 工具包。

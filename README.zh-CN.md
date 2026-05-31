# difft-dir-viewer

[English](README.md) | [简体中文](README.zh-CN.md)

基于 [difftastic](https://github.com/wilfred/difftastic) 的 Slint GUI，用于对比 **两个目录**。

Viewer 通过子进程调用 `difft`，读取 `--display json` 输出，左侧展示 **Changed files** 列表，右侧双栏显示当前文件的 diff。

## 基本机制

```
┌─────────────────┐     subprocess      ┌──────────┐
│ difft-dir-      │  DFT_UNSTABLE=yes   │  difft   │
│ viewer (Slint)  │ ──────────────────► │  (CLI)   │
└─────────────────┘  --display json     └──────────┘
        │                                      │
        │    stdout: JSON 数组（多个 object）   │
        └──────────────────────────────────────┘
```

1. 启动时传入目录 A、目录 B（**必须**两个命令行参数）。
2. Viewer 执行：

   ```text
   difft --display json --byte-limit 32000000 --context 3 <dirA> <dirB>
   ```

3. 解析 JSON **数组** — 每个变更文件一项（默认不含 unchanged）。
4. **Changed files** 列表显示相对路径与状态标记：
   - **M** — 修改（两侧都有）
   - **A** — 新增（仅在 B）
   - **D** — 删除（仅在 A）
5. 点击文件名切换右侧 diff 内容。

### 多层目录

`difft` 递归遍历子目录。JSON 中 `path` 为 **相对所选根目录的路径**，例如：

```text
src/main.rs
sub/deep/f.rs
.hidden/doc.txt
```

侧栏显示完整相对路径（列宽 240px，过长可横向滚动）。**无目录树**，仅为 `difft` 返回顺序的扁平列表。

### 新增 / 删除文件

**A** / **D** 条目常返回空的 `aligned_lines`。Viewer 会从目录 A 或 B 读盘，填充单侧代码栏。

## 环境要求

| 组件 | 说明 |
|------|------|
| Rust | 1.85+ |
| `difft` | 建议与本 crate 同版本（当前 **0.70.0**） |
| 系统 | macOS、Linux、Windows |

Viewer 会在子进程中设置 `DFT_UNSTABLE=yes`（JSON 输出在上游为实验性功能）。

## 平台支持

本 crate **跨平台**（macOS、Linux、Windows），viewer 本身没有仅 Unix 可用的 GUI 或 I/O 路径。

| 方面 | Windows 处理（源码） |
|------|----------------------|
| GUI | Slint `backend-winit` + `renderer-femtovg`（支持 Windows） |
| Release 二进制 | `windows_subsystem = "windows"` — 启动时不额外弹出控制台 |
| 查找 `difft` | `difft.exe` 名称、`where difft`、`DIFT_PATH` 可带 `.exe` 后缀 |
| 子进程 | `CREATE_NO_WINDOW`，避免弹出控制台（`difft_probe.rs`） |
| 路径 / CLI | `std::path` + `args_os()`，无硬编码 `/` 分隔符 |
| 安装提示 | `install_message()` 含 Windows 说明（`winget`、`scoop`、`choco`） |

工作区 CI 矩阵包含 `x86_64-pc-windows-msvc` 与 `aarch64-pc-windows-msvc`，作为 workspace 成员会在 Windows 上参与 `cargo test` 构建。

## 构建

在仓库根目录：

```bash
cargo build -p difftastic -p difft-dir-viewer
```

产物：

- `target/debug/difft`（Windows 为 `.exe`）
- `target/debug/difft-dir-viewer`（Windows 为 `.exe`）

Windows（PowerShell 或 cmd）命令相同：

```powershell
cargo build -p difftastic -p difft-dir-viewer
set DIFT_PATH=%CD%\target\debug\difft.exe
target\debug\difft-dir-viewer.exe dir-a dir-b
```

Release：

```bash
cargo build --release -p difftastic -p difft-dir-viewer
```

## 安装 `difft`

查找顺序：

1. 环境变量 `DIFT_PATH`
2. 与 viewer 同目录下的 `difft` / `difft.exe`
3. `PATH` 中的 `difft`

未找到时，界面会显示安装提示。

**macOS**

```bash
brew install difftastic
export DIFT_PATH="$(pwd)/target/debug/difft"   # 本地构建时
```

**Windows**

```powershell
winget install Wilfred.difftastic
set DIFT_PATH=%CD%\target\debug\difft.exe
```

**Linux** — 使用发行版包管理器，或源码构建后设置 `DIFT_PATH`。

验证：`difft --version`

## 使用

```bash
cargo run -p difft-dir-viewer -- sample_files/dir_1 sample_files/dir_2
```

**必须**提供两个目录路径；启动后自动 diff，顶栏仅显示路径（不可点击选择）。

Release 二进制：

```bash
difft-dir-viewer /path/to/dir-a /path/to/dir-b
```

## 界面布局（简要）

- 顶栏：两个目录路径（只读显示）。
- 紫色区域：仅错误或警告（成功 diff 且无警告时隐藏）。
- 左侧：**Changed files (N)** 深色面板。
- 右侧：双栏代码 diff。

## 快捷键

焦点需在右侧 diff 代码区（切换文件后会自动聚焦）。快捷键沿用常见 Vim 风格。

在 macOS 上，下表中的 **Ctrl** 对相同操作也匹配 **⌘ (Meta)**。

### 滚动

| 按键 | 作用 |
|------|------|
| `Page Up` | 向上翻一整页 |
| `Page Down` | 向下翻一整页 |
| `Ctrl+b` | 向上翻一整页 |
| `Ctrl+f` | 向下翻一整页 |
| `Ctrl+u` | 向上翻半页 |
| `Ctrl+d` | 向下翻半页 |
| `Home` | 滚到文件顶部 |
| `End` | 滚到文件底部 |
| `G` 或 `Shift+g` | 滚到文件底部 |
| `g` 再 `g` | 滚到文件顶部（连按两次 `g`） |
| `h` | 代码区**向左**滚（长行；行号 gutter 固定） |
| `l` | 代码区**向右**滚 |
| 触摸板 / 滚轮（代码区） | 以水平滑动为主时，同步横向滚动 |

行号在固定 **gutter** 列；仅代码 pane 水平滚动。

**Changed files** 侧栏：路径可横向滚动（状态列固定）；点击侧栏后，上述快捷键作用于文件列表，点击代码区后作用于 diff 代码。

## JSON 格式（重要）

目录 diff 的 stdout 为 **JSON 数组**（不是单个 object）。每项包含：

- `path`、`language`、`status`（`changed` / `created` / `deleted` / `unchanged`）
- `extra_info`（可选）
- `aligned_lines[]`：`lhs_text`、`rhs_text`、`is_novel_lhs`、`is_novel_rhs` 及可选 span 信息

上游明确 `--display json` 为 **实验性**：

- 需要 `DFT_UNSTABLE=yes`。
- 输出 **可能变更**，且无版本号字段。
- **不是** semver 稳定的公开 API。

Viewer 会解析完整数组；尽量忽略未知字段；必需字段缺失或重命名会导致失败。

**部署时请将 `difft` 与 viewer 固定在同一 git revision。**

## 行为说明

| 项目 | 说明 |
|------|------|
| 输入 | 仅 **目录** — 选择文件会报错 |
| 多层目录 | 由 `difft` 递归；列表显示 `dir/file` 形式路径 |
| 列表 | 扁平列表，无分组/折叠 |
| 大仓库 | 变更文件多 → 侧栏很长；性能取决于 `difft` |
| 忽略规则 | 与 CLI 相同（如 `.gitignore`），viewer 不可配置 |
| 文件大小 | 单文件 `--byte-limit 32000000`（32 MiB） |
| 长行 | 代码区可横向滚动（`h` / `l` 或触摸板）；行号 gutter 固定 |
| Changed files 长路径 | 路径可横向滚动；点击侧栏后 `h`/`l`/翻页键作用于文件列表 |

## 故障排查

| 现象 | 可能原因 |
|------|----------|
| `Path A is not a directory` | 请选择目录，不要选单个文件 |
| Changed files 为空 | 无差异或全部 unchanged |
| A/D 文件内容区空白 | 读盘失败 — 看紫色提示；二进制可能无法 `read_to_string` |
| `difft not found` | 设置 `DIFT_PATH` 或安装 difftastic |
| JSON 解析失败 | `difft` 版本不匹配 — 同仓库重建 |

## 许可证

MIT — 与 difftastic 相同。

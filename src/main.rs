#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// Release Windows builds use the GUI subsystem (no extra console window).

mod difft_probe;
mod line_ending;
mod model;
mod segments;
#[cfg(target_os = "macos")]
mod macos_edge;
#[cfg(target_os = "macos")]
mod macos_icon;
#[cfg(target_os = "windows")]
mod windows_edge;

slint::include_modules!();

use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use difft_probe::{difft_command, install_message, resolve_difft};
use model::{display_lines, file_info_message, parse_diff_results, status_label, DiffFile, DisplayLine};
use segments::{
    build_segments, code_brush, plain_line_brush, prepare_display_line, text_pixel_width,
    to_slint_segments, Side, GUTTER_LINE,
};

const BYTE_LIMIT: &str = "32000000";
const PARSE_ERROR_LIMIT: &str = "4096";
/// Show essentially the whole file in the GUI (not just changed hunks).
const FULL_FILE_CONTEXT: &str = "999999";

struct CliDirs {
    path_a: PathBuf,
    path_b: PathBuf,
    label_a: String,
    label_b: String,
    difft: Option<PathBuf>,
}

struct DiffSession {
    path_a: PathBuf,
    path_b: PathBuf,
    files: Vec<DiffFile>,
}

fn usage() -> String {
    "Usage: difft-dir-viewer [--difft PATH] <dir-a> <dir-b>\n\
     \n\
     Options:\n\
       --difft PATH   Path to the difft binary (overrides DIFT_PATH and auto-discovery)\n\
       -h, --help     Show this help"
        .to_owned()
}

/// Build a usage error message for an invalid CLI argument count.
fn cli_usage_error(got: usize) -> String {
    let detail = match got {
        0 => "two directory paths are required.".to_string(),
        1 => "two directory paths are required (got 1).".to_string(),
        n => format!("expected exactly 2 directory paths (got {n})."),
    };
    format!("{}\n\nError: {detail}", usage())
}

/// Parse exactly two directory paths from the command line.
fn parse_cli_directories() -> Result<CliDirs, String> {
    let mut difft = None;
    let mut paths = Vec::new();
    let mut args = env::args_os().skip(1);

    while let Some(arg) = args.next() {
        let key = arg.to_string_lossy();
        match key.as_ref() {
            "--help" | "-h" => return Err(usage()),
            "--difft" => {
                let Some(value) = args.next() else {
                    return Err(format!("--difft requires a path.\n\n{}", usage()));
                };
                difft = Some(PathBuf::from(value));
            }
            _ if key.starts_with("--difft=") => {
                let path = key.trim_start_matches("--difft=");
                if path.is_empty() {
                    return Err(format!("--difft requires a path.\n\n{}", usage()));
                }
                difft = Some(PathBuf::from(path));
            }
            _ if key.starts_with('-') => {
                return Err(format!("unknown option: {key}\n\n{}", usage()));
            }
            _ => paths.push(PathBuf::from(arg)),
        }
    }

    match paths.len() {
        2 => Ok(CliDirs {
            path_a: resolve_path(paths[0].clone()),
            path_b: resolve_path(paths[1].clone()),
            label_a: paths[0].to_string_lossy().into_owned(),
            label_b: paths[1].to_string_lossy().into_owned(),
            difft,
        }),
        got => Err(cli_usage_error(got)),
    }
}

/// Resolve a user path for filesystem / subprocess use (canonical when possible).
fn resolve_path(path: PathBuf) -> PathBuf {
    let absolute = if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(&path))
            .unwrap_or(path)
    };

    absolute.canonicalize().unwrap_or(absolute)
}

/// Run `difft` on two directories and parse the JSON array of changed files.
fn run_difft(difft: &Path, path_a: &Path, path_b: &Path) -> Result<Vec<DiffFile>, String> {
    let output = difft_command(difft)
        .env("DFT_UNSTABLE", "yes")
        .env("DFT_PARSE_ERROR_LIMIT", PARSE_ERROR_LIMIT)
        .args([
            "--display",
            "json",
            "--byte-limit",
            BYTE_LIMIT,
            "--context",
            FULL_FILE_CONTEXT,
        ])
        .arg(path_a)
        .arg(path_b)
        .output()
        .map_err(|e| format!("failed to run {}: {e}", difft.display()))?;

    if !output.stdout.is_empty() {
        return parse_diff_results(&output.stdout, path_a, path_b);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        return Err(stderr.trim().to_owned());
    }

    Err(format!(
        "difft exited with status {} and produced no output.",
        output.status
    ))
}

/// Difft JSON line numbers are 0-based file indices; display 1-based like the terminal.
fn line_num(n: Option<u32>) -> i32 {
    n.map(|n| (n + 1) as i32).unwrap_or(-1)
}

/// Map a `DisplayLine` into a Slint `DiffLine` with syntax segments.
fn slint_line(line: &DisplayLine) -> DiffLine {
    let (lhs_text, lhs_spans) = prepare_display_line(&line.lhs_text, &line.lhs_spans);
    let (rhs_text, rhs_spans) = prepare_display_line(&line.rhs_text, &line.rhs_spans);
    DiffLine {
        lhs_novel: line.is_novel_lhs,
        rhs_novel: line.is_novel_rhs,
        lhs_line: line_num(line.lhs_line),
        rhs_line: line_num(line.rhs_line),
        lhs_plain_text: lhs_text.clone().into(),
        rhs_plain_text: rhs_text.clone().into(),
        lhs_plain_color: plain_line_brush(line.is_novel_lhs, Side::Left),
        rhs_plain_color: plain_line_brush(line.is_novel_rhs, Side::Right),
        lhs_segments: to_slint_segments(&build_segments(
            &lhs_text,
            &lhs_spans,
            line.is_novel_lhs,
            Side::Left,
        )),
        rhs_segments: to_slint_segments(&build_segments(
            &rhs_text,
            &rhs_spans,
            line.is_novel_rhs,
            Side::Right,
        )),
        lhs_content_width: text_pixel_width(&lhs_text),
        rhs_content_width: text_pixel_width(&rhs_text),
    }
}

/// Widest code line in the diff panes, for horizontal scroll sizing.
fn max_line_content_width(lines: &[DiffLine]) -> f32 {
    lines
        .iter()
        .fold(0.0f32, |max_width, line| {
            max_width.max(line.lhs_content_width).max(line.rhs_content_width)
        })
}

/// Widest changed-file path in the sidebar, for horizontal scroll sizing.
fn max_file_path_width(files: &[DiffFile]) -> f32 {
    files
        .iter()
        .fold(0.0f32, |max_width, file| max_width.max(text_pixel_width(&file.path)))
}

/// Build Slint diff lines for one changed file (reads disk for A/D entries).
fn slint_lines(
    file: &DiffFile,
    path_a: &Path,
    path_b: &Path,
) -> Result<Vec<DiffLine>, String> {
    display_lines(file, path_a, path_b).map(|lines| lines.iter().map(slint_line).collect())
}

/// Move keyboard focus to the side-by-side code panel.
fn focus_diff_panel(ui: &MainWindow) {
    ui.invoke_focus_diff_panel();
}

/// Defer code-panel focus until after the UI has finished updating.
fn schedule_focus_diff_panel(ui: &MainWindow) {
    let ui_handle = ui.as_weak();
    let _ = slint::Timer::single_shot(Duration::from_millis(50), move || {
        if let Some(ui) = ui_handle.upgrade() {
            ui.invoke_focus_diff_panel();
        }
    });
}

/// Render one changed file in the diff panes and update selection state.
fn show_diff_file(ui: &MainWindow, file: &DiffFile, index: i32, path_a: &Path, path_b: &Path) {
    match slint_lines(file, path_a, path_b) {
        Ok(lines) => {
            let count = lines.len();
            ui.set_max_content_width(max_line_content_width(&lines));
            let model: slint::ModelRc<DiffLine> =
                std::rc::Rc::new(slint::VecModel::from(lines)).into();
            ui.set_lines(model);
            ui.set_selected_file_index(index);
            ui.set_file_info(file_info_message(file, count).into());
            focus_diff_panel(ui);
        }
        Err(err) => {
            ui.set_max_content_width(0.0);
            ui.set_lines(slint::ModelRc::new(slint::VecModel::from(Vec::<DiffLine>::new())));
            ui.set_selected_file_index(index);
            ui.set_file_info(err.into());
            focus_diff_panel(ui);
        }
    }
}

/// Reset diff panes and the changed-files list to an empty state.
fn clear_diff_view(ui: &MainWindow) {
    ui.set_max_content_width(0.0);
    ui.set_max_file_path_width(0.0);
    ui.set_lines(slint::ModelRc::new(slint::VecModel::from(Vec::<DiffLine>::new())));
    ui.set_changed_files(slint::ModelRc::new(slint::VecModel::from(
        Vec::<DiffFileEntry>::new(),
    )));
    ui.set_changed_file_count(0);
    ui.set_selected_file_index(-1);
    ui.set_file_list_title("".into());
    ui.set_file_info("".into());
}

/// Populate the changed-files sidebar and highlight the selected row.
fn update_file_list(ui: &MainWindow, files: &[DiffFile], selected: i32) {
    let entries: Vec<DiffFileEntry> = files
        .iter()
        .map(|file| DiffFileEntry {
            path: file.path.clone().into(),
            status: status_label(file.status).into(),
        })
        .collect();
    ui.set_max_file_path_width(max_file_path_width(files));
    ui.set_changed_files(std::rc::Rc::new(slint::VecModel::from(entries)).into());
    ui.set_changed_file_count(files.len() as i32);
    ui.set_file_list_title(format!("Changed files ({})", files.len()).into());
    ui.set_selected_file_index(selected);
}

/// Show the full directory diff: file list plus the first changed file.
fn show_diff_results(ui: &MainWindow, session: &DiffSession) {
    if session.files.is_empty() {
        clear_diff_view(ui);
        return;
    }

    update_file_list(ui, &session.files, 0);
    show_diff_file(&ui, &session.files[0], 0, &session.path_a, &session.path_b);
}

/// Ensure both CLI paths exist and are directories.
fn validate_directories(path_a: &Path, path_b: &Path) -> Result<(), String> {
    if !path_a.is_dir() {
        return Err(format!(
            "Path A is not a directory: {}",
            path_a.display()
        ));
    }
    if !path_b.is_dir() {
        return Err(format!(
            "Path B is not a directory: {}",
            path_b.display()
        ));
    }
    Ok(())
}

/// Run directory diff on a background thread and update the UI on completion.
fn run_diff(
    ui_handle: slint::Weak<MainWindow>,
    difft: Arc<Mutex<Option<PathBuf>>>,
    path_a: PathBuf,
    path_b: PathBuf,
    diff_session: Arc<Mutex<Option<DiffSession>>>,
) {
    let difft_path = match difft.lock().unwrap().clone() {
        Some(path) => path,
        None => {
            if let Some(ui) = ui_handle.upgrade() {
                ui.set_status_text("difft not found.".into());
                ui.set_file_info(install_message().into());
            }
            return;
        }
    };

    if let Err(err) = validate_directories(&path_a, &path_b) {
        if let Some(ui) = ui_handle.upgrade() {
            ui.set_status_text("".into());
            ui.set_file_info(err.into());
        }
        return;
    }

    if let Some(ui) = ui_handle.upgrade() {
        ui.set_status_text("Diffing...".into());
        ui.set_file_info("".into());
    }

    std::thread::spawn(move || {
        let outcome = run_difft(&difft_path, &path_a, &path_b);
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_handle.upgrade() {
                match outcome {
                    Ok(files) => {
                        let session = DiffSession {
                            path_a: path_a.clone(),
                            path_b: path_b.clone(),
                            files,
                        };
                        show_diff_results(&ui, &session);
                        *diff_session.lock().unwrap() = Some(session);
                        ui.set_status_text("".into());
                        focus_diff_panel(&ui);
                    }
                    Err(err) => {
                        *diff_session.lock().unwrap() = None;
                        clear_diff_view(&ui);
                        ui.set_status_text("".into());
                        ui.set_file_info(err.into());
                    }
                }
            }
        });
    });
}

fn init_gutter_colors(ui: &MainWindow) {
    ui.set_gutter_line_color(code_brush(GUTTER_LINE));
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
fn schedule_fill_screen_edges(ui: &MainWindow) {
    let ui_handle = ui.as_weak();
    for delay_ms in [0_u64, 50, 200, 500, 1000] {
        let ui_handle = ui_handle.clone();
        slint::Timer::single_shot(Duration::from_millis(delay_ms), move || {
            if let Some(ui) = ui_handle.upgrade() {
                let window = ui.window();
                #[cfg(target_os = "windows")]
                windows_edge::fill_work_area(&window);
                #[cfg(target_os = "macos")]
                macos_edge::fill_screen(&window);
            }
        });
    }
}

/// Maximize the window on startup and schedule initial focus.
fn maximize_on_startup(ui: &MainWindow) {
    let ui_handle = ui.as_weak();
    slint::Timer::single_shot(Duration::from_millis(0), move || {
        if let Some(ui) = ui_handle.upgrade() {
            let window = ui.window();
            #[cfg(target_os = "windows")]
            {
                windows_edge::fill_work_area(&window);
                windows_edge::install_borderless_hooks(&window);
            }
            #[cfg(target_os = "macos")]
            macos_edge::fill_screen(&window);
            #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
            {
                window.set_maximized(true);
            }
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            schedule_fill_screen_edges(&ui);
            schedule_focus_diff_panel(&ui);
        }
    });
}

#[cfg(target_os = "macos")]
fn schedule_application_icon() {
    slint::Timer::single_shot(Duration::from_millis(0), || {
        macos_icon::set_from_png(include_bytes!("../assets/icons/icon-512.png"));
    });
}

/// Application entry: parse CLI, wire callbacks, and start the Slint event loop.
fn main() -> Result<(), slint::PlatformError> {
    let dirs = parse_cli_directories();
    let ui = MainWindow::new()?;
    init_gutter_colors(&ui);
    maximize_on_startup(&ui);
    #[cfg(target_os = "macos")]
    schedule_application_icon();
    clear_diff_view(&ui);

    if let Ok(dirs) = &dirs {
        ui.set_path_a(dirs.label_a.clone().into());
        ui.set_path_b(dirs.label_b.clone().into());
    }

    let difft_path = dirs.as_ref().ok().and_then(|dirs| dirs.difft.clone());
    let difft = Arc::new(Mutex::new(resolve_difft(difft_path).ok()));
    let diff_session: Arc<Mutex<Option<DiffSession>>> = Arc::new(Mutex::new(None));

    match (&dirs, difft.lock().unwrap().as_ref()) {
        (Err(err), _) => {
            ui.set_file_info(err.clone().into());
        }
        (_, None) => {
            ui.set_status_text("difft not found.".into());
            ui.set_file_info(install_message().into());
        }
        (Ok(_), Some(_)) => {
            ui.set_status_text("".into());
        }
    }

    {
        let ui_handle = ui.as_weak();
        let diff_session_store = Arc::clone(&diff_session);
        ui.on_file_selected(move |index| {
            if let Some(ui) = ui_handle.upgrade() {
                let session = diff_session_store.lock().unwrap();
                if let Some(session) = session.as_ref() {
                    if let Some(file) = session.files.get(index as usize) {
                        show_diff_file(
                            &ui,
                            file,
                            index,
                            &session.path_a,
                            &session.path_b,
                        );
                    }
                }
            }
        });
    }

    if let (Ok(dirs), true) = (&dirs, difft.lock().unwrap().is_some()) {
        let ui_handle = ui.as_weak();
        let difft_store = Arc::clone(&difft);
        let path_a = dirs.path_a.clone();
        let path_b = dirs.path_b.clone();
        let diff_session_store = Arc::clone(&diff_session);
        slint::Timer::single_shot(Duration::from_millis(0), move || {
            run_diff(
                ui_handle,
                difft_store,
                path_a,
                path_b,
                diff_session_store,
            );
        });
    }

    ui.on_quit_requested(move || {
        let _ = slint::quit_event_loop();
    });

    ui.run()
}

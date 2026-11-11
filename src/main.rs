#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use clap::Parser;
use image::{DynamicImage, GenericImageView};
use ort::{
    execution_providers::{CPUExecutionProvider, CUDAExecutionProvider, DirectMLExecutionProvider},
    inputs,
    session::{builder::GraphOptimizationLevel, Session},
    value::{Value, ValueType},
};
use slint::ComponentHandle;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use walkdir::WalkDir;

const MODEL_NAME: &str = "birefnetv2-img.onnx";
const SUPPORTED_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "webp", "avif", "heic", "heif"];

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn CreateMutexW(
        attributes: *const std::ffi::c_void,
        initial_owner: i32,
        name: *const u16,
    ) -> *mut std::ffi::c_void;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, milliseconds: u32) -> u32;
    fn ReleaseMutex(handle: *mut std::ffi::c_void) -> i32;
    fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
}

slint::slint! {
    import { Button, HorizontalBox, ProgressIndicator, ScrollView, VerticalBox } from "std-widgets.slint";

    export component MainWindow inherits Window {
        title: "Background Remover AI";
        icon: @image-url("../assets/app_icon.png");
        preferred-width: 840px;
        preferred-height: 860px;
        min-width: 740px;
        min-height: 760px;

        in-out property <int> file-count: 0;
        in-out property <string> files-text: "No images selected";
        in-out property <string> output-dir: "Same folder as input";
        in-out property <string> status-text: "Ready";
        in-out property <int> progress-value: 0;
        in-out property <bool> busy: false;
        in-out property <bool> drag-active: false;
        in property <string> model-status: "";

        callback add-files();
        callback add-folder();
        callback clear-files();
        callback choose-output();
        callback start-processing();
        callback install-context-menu();

        background: rgb(20, 23, 28);

        VerticalBox {
            padding: 24px;
            spacing: 15px;

            HorizontalBox {
                spacing: 14px;
                Image {
                    source: @image-url("../assets/app_icon.png");
                    width: 58px;
                    height: 58px;
                    image-fit: contain;
                }
                VerticalBox {
                    spacing: 3px;
                    Text {
                        text: "AI Background Remover";
                        color: rgb(245, 247, 250);
                        font-size: 28px;
                        font-weight: 700;
                        vertical-alignment: center;
                    }
                    Text {
                        text: root.model-status;
                        color: rgb(174, 182, 194);
                        font-size: 13px;
                        vertical-alignment: center;
                    }
                }
            }

            Rectangle {
                height: 230px;
                border-radius: 16px;
                border-width: 2px;
                border-color: root.drag-active ? rgb(96, 196, 171) : rgb(66, 73, 85);
                background: root.drag-active ? rgb(27, 49, 46) : rgb(28, 32, 38);

                VerticalBox {
                    padding: 25px;
                    spacing: 9px;
                    alignment: center;

                    Text {
                        text: root.drag-active ? "Drop image or folder here" : "Drag & drop images here";
                        color: rgb(244, 246, 248);
                        font-size: 21px;
                        horizontal-alignment: center;
                    }
                    Text {
                        text: "JPG  •  PNG  •  WEBP  •  AVIF  •  HEIC  •  HEIF";
                        color: rgb(155, 165, 178);
                        font-size: 13px;
                        horizontal-alignment: center;
                    }
                    HorizontalBox {
                        spacing: 10px;
                        alignment: center;
                        Button {
                            text: "Choose images";
                            enabled: !root.busy;
                            clicked => { root.add-files(); }
                        }
                        Button {
                            text: "Add folder";
                            enabled: !root.busy;
                            clicked => { root.add-folder(); }
                        }
                    }
                }
            }

            HorizontalBox {
                spacing: 10px;
                Text {
                    text: root.file-count + " image(s)";
                    color: rgb(232, 235, 239);
                    vertical-alignment: center;
                }
                Rectangle { horizontal-stretch: 1; }
                Button {
                    text: "Clear";
                    enabled: !root.busy && root.file-count > 0;
                    clicked => { root.clear-files(); }
                }
            }

            ScrollView {
                height: 165px;
                Text {
                    text: root.files-text;
                    color: rgb(198, 204, 213);
                    font-size: 12px;
                    wrap: word-wrap;
                }
            }

            Rectangle {
                height: 50px;
                border-radius: 9px;
                background: rgb(28, 32, 38);
                HorizontalBox {
                    padding: 10px;
                    spacing: 10px;
                    Text {
                        text: "Save to:";
                        color: rgb(220, 225, 231);
                        vertical-alignment: center;
                    }
                    Text {
                        text: root.output-dir;
                        color: rgb(174, 182, 194);
                        overflow: elide;
                        vertical-alignment: center;
                        horizontal-stretch: 1;
                    }
                    Button {
                        text: "Choose folder";
                        enabled: !root.busy;
                        clicked => { root.choose-output(); }
                    }
                }
            }

            Rectangle {
                height: 70px;
                border-radius: 10px;
                background: rgb(25, 29, 35);
                VerticalBox {
                    padding: 11px;
                    spacing: 4px;
                    Text {
                        text: "Output";
                        color: rgb(231, 235, 240);
                        font-size: 14px;
                        font-weight: 600;
                    }
                    Text {
                        text: "Transparent PNG: original-name_nobg.png";
                        color: rgb(158, 168, 181);
                        font-size: 12px;
                    }
                }
            }

            ProgressIndicator {
                progress: root.progress-value / 100.0;
            }

            Text {
                text: root.status-text;
                color: root.busy ? rgb(142, 192, 255) : rgb(174, 182, 194);
                font-size: 13px;
                wrap: word-wrap;
            }

            HorizontalBox {
                spacing: 10px;
                Button {
                    text: "Install Explorer Actions menu";
                    enabled: !root.busy;
                    clicked => { root.install-context-menu(); }
                }
                Rectangle { horizontal-stretch: 1; }
                Button {
                    text: root.busy ? "Removing background..." : "Remove background and save";
                    enabled: !root.busy && root.file-count > 0;
                    clicked => { root.start-processing(); }
                }
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about = "AI image background remover with Slint GUI and Explorer Actions")]
struct Args {
    /// File(s) or folder(s) to process. If omitted, the Slint GUI is opened.
    #[arg(index = 1, value_name = "INPUT", num_args = 0..)]
    input: Vec<PathBuf>,

    /// Install Actions > Remove background in Windows Explorer.
    #[arg(long)]
    add: bool,

    /// Remove Explorer Actions entries installed by this app.
    #[arg(long)]
    remove_context_menu: bool,

    /// Internal flag used by Explorer Actions. Displays processing errors in a dialog.
    #[arg(long, hide = true)]
    context_menu: bool,

    /// Optional output directory. By default output is saved next to each input image.
    #[arg(long)]
    output_dir: Option<PathBuf>,

    /// Force CPU execution.
    #[arg(long)]
    cpu: bool,

    /// Retained for command-line compatibility. Multi-image processing uses one model session and queues images in pairs.
    #[arg(short = 't', long, default_value_t = 1)]
    threads: usize,
}

#[cfg(target_os = "windows")]
fn configure_process_priority() {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, SetPriorityClass, IDLE_PRIORITY_CLASS,
    };
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), IDLE_PRIORITY_CLASS);
    }
}

#[cfg(not(target_os = "windows"))]
fn configure_process_priority() {}

#[cfg(target_os = "windows")]
fn attach_parent_console() {
    use windows_sys::Win32::System::Console::{AttachConsole, ATTACH_PARENT_PROCESS};
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_parent_console() {}

fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|s| s.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn collect_image_files(input: &Path) -> Vec<PathBuf> {
    if input.is_file() {
        return if is_supported_image(input) {
            vec![input.to_path_buf()]
        } else {
            Vec::new()
        };
    }

    if input.is_dir() {
        return WalkDir::new(input)
            .into_iter()
            .filter_map(Result::ok)
            .map(|entry| entry.into_path())
            .filter(|path| path.is_file() && is_supported_image(path))
            .collect();
    }

    Vec::new()
}

fn add_path_to_list(files: &mut Vec<PathBuf>, path: &Path) {
    for candidate in collect_image_files(path) {
        if !files.contains(&candidate) {
            files.push(candidate);
        }
    }
    files.sort();
}

fn prepare_input_list(inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for input in inputs {
        add_path_to_list(&mut files, input);
    }
    files.sort();
    files.dedup();
    files
}

fn format_file_list(files: &[PathBuf]) -> String {
    if files.is_empty() {
        return "No images selected".to_string();
    }

    files
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn sync_gui_file_state(ui: &MainWindow, files: &[PathBuf]) {
    ui.set_file_count(files.len() as i32);
    ui.set_files_text(format_file_list(files).into());
}

#[cfg(feature = "integrated")]
fn model_status_text() -> String {
    format!("AI model: {} embedded in this executable", MODEL_NAME)
}

#[cfg(all(not(feature = "integrated"), target_os = "windows"))]
fn model_status_text() -> String {
    match ai_manager_model_path() {
        Ok(path) if path.exists() => format!("AI Models Manager: {}", path.display()),
        Ok(path) => format!("AI Models Manager entry found, but file is missing: {}", path.display()),
        Err(_) => format!("AI Models Manager: {} not registered", MODEL_NAME),
    }
}

#[cfg(all(not(feature = "integrated"), not(target_os = "windows")))]
fn model_status_text() -> String {
    format!("External model required: set BACKGROUND_REMOVER_MODEL to {}", MODEL_NAME)
}

#[cfg(target_os = "windows")]
fn ai_manager_model_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let key = hklm.open_subkey(r#"SOFTWARE\Gallery Inc\AI-ModelsManager\models"#)?;
    let value: String = key.get_value(MODEL_NAME)?;
    Ok(PathBuf::from(value))
}

#[cfg(all(not(feature = "integrated"), not(target_os = "windows")))]
fn external_model_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(PathBuf::from(std::env::var("BACKGROUND_REMOVER_MODEL")?))
}

fn create_session(force_cpu: bool) -> Result<Session, Box<dyn std::error::Error>> {
    let _ = ort::init().with_name("rustbackremover").commit();

    let mut builder = Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::Level1)?
        .with_intra_threads(2)?;

    if force_cpu {
        builder = builder.with_execution_providers([CPUExecutionProvider::default().build()])?;
    } else {
        builder = builder.with_execution_providers([
            CUDAExecutionProvider::default().build(),
            DirectMLExecutionProvider::default().build(),
            CPUExecutionProvider::default().build(),
        ])?;
    }

    #[cfg(feature = "integrated")]
    {
        const MODEL_BYTES: &[u8] = include_bytes!("../birefnetv2-img.onnx");
        return Ok(builder.commit_from_memory(MODEL_BYTES)?);
    }

    #[cfg(all(not(feature = "integrated"), target_os = "windows"))]
    {
        let model_path = ai_manager_model_path()?;
        if !model_path.exists() {
            return Err(format!("AI model does not exist at: {}", model_path.display()).into());
        }
        return Ok(builder.commit_from_file(model_path)?);
    }

    #[cfg(all(not(feature = "integrated"), not(target_os = "windows")))]
    {
        let model_path = external_model_path()?;
        if !model_path.exists() {
            return Err(format!("AI model does not exist at: {}", model_path.display()).into());
        }
        Ok(builder.commit_from_file(model_path)?)
    }
}

#[cfg(target_os = "windows")]
fn register_context_menu() -> Result<(), Box<dyn std::error::Error>> {
    use winreg::enums::*;
    use winreg::RegKey;

    fn install_action(
        hkcu: &RegKey,
        base: &str,
        exe_path: &str,
        exe_icon: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let _ = hkcu.delete_subkey_all(base);

        let (actions_key, _) = hkcu.create_subkey(base)?;
        actions_key.set_value("MUIVerb", &"Actions")?;
        actions_key.set_value("Icon", &exe_icon)?;
        actions_key.set_value("SubCommands", &"")?;
        actions_key.set_value("MultiSelectModel", &"Player")?;

        let sub_path = format!(r#"{}\shell\01_RemoveBackground"#, base);
        let (sub_key, _) = hkcu.create_subkey(&sub_path)?;
        sub_key.set_value("", &"Remove background")?;
        sub_key.set_value("MUIVerb", &"Remove background")?;
        sub_key.set_value("Icon", &exe_icon)?;
        sub_key.set_value("MultiSelectModel", &"Player")?;

        let (command_key, _) = sub_key.create_subkey("command")?;
        command_key.set_value("", &format!("\"{}\" --context-menu -- \"%1\"", exe_path))?;
        Ok(())
    }

    let exe_path = std::env::current_exe()?.display().to_string();
    let exe_icon = format!("{},0", exe_path);
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    for ext in [".jpg", ".jpeg", ".png", ".webp", ".avif", ".heic", ".heif"] {
        let old = format!(
            r#"Software\Classes\SystemFileAssociations\{}\shell\RemoveBackground"#,
            ext
        );
        let base = format!(
            r#"Software\Classes\SystemFileAssociations\{}\shell\ImageBackgroundRemoverActions"#,
            ext
        );
        let _ = hkcu.delete_subkey_all(&old);
        install_action(&hkcu, &base, &exe_path, &exe_icon)?;
    }

    // Folder support uses the same batch engine as files, UI and CLI.
    install_action(
        &hkcu,
        r#"Software\Classes\Directory\shell\ImageBackgroundRemoverActions"#,
        &exe_path,
        &exe_icon,
    )?;

    notify_shell_association_changed();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn register_context_menu() -> Result<(), Box<dyn std::error::Error>> {
    Err("Explorer context menu registration is only available on Windows".into())
}

#[cfg(target_os = "windows")]
fn unregister_context_menu() -> Result<(), Box<dyn std::error::Error>> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    for ext in [".jpg", ".jpeg", ".png", ".webp", ".avif", ".heic", ".heif"] {
        let old = format!(
            r#"Software\Classes\SystemFileAssociations\{}\shell\RemoveBackground"#,
            ext
        );
        let base = format!(
            r#"Software\Classes\SystemFileAssociations\{}\shell\ImageBackgroundRemoverActions"#,
            ext
        );
        let _ = hkcu.delete_subkey_all(old);
        let _ = hkcu.delete_subkey_all(base);
    }
    let _ = hkcu.delete_subkey_all(
        r#"Software\Classes\Directory\shell\ImageBackgroundRemoverActions"#,
    );
    notify_shell_association_changed();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn unregister_context_menu() -> Result<(), Box<dyn std::error::Error>> {
    Err("Explorer context menu registration is only available on Windows".into())
}

#[cfg(target_os = "windows")]
fn notify_shell_association_changed() {
    use std::ffi::c_void;

    const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
    const SHCNF_IDLIST: u32 = 0x0000;

    #[link(name = "shell32")]
    extern "system" {
        fn SHChangeNotify(
            w_event_id: i32,
            u_flags: u32,
            item1: *const c_void,
            item2: *const c_void,
        );
    }

    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED,
            SHCNF_IDLIST,
            std::ptr::null(),
            std::ptr::null(),
        );
    }
}

fn output_directory_for(input: &Path, chosen: Option<&Path>) -> PathBuf {
    chosen
        .map(Path::to_path_buf)
        .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")).to_path_buf())
}

#[derive(Debug, Clone)]
struct BatchFailure {
    path: PathBuf,
    error: String,
}

#[derive(Debug, Clone, Default)]
struct BatchSummary {
    total: usize,
    succeeded: usize,
    failures: Vec<BatchFailure>,
}

#[derive(Debug, Clone)]
enum ProcessingEvent {
    LoadingModel,
    ListReady {
        total: usize,
        batches: usize,
    },
    ImageStarted {
        index: usize,
        total: usize,
        batch: usize,
        batches: usize,
        path: PathBuf,
    },
    ImageFinished {
        index: usize,
        total: usize,
        succeeded: usize,
        failed: usize,
    },
    UnloadingModel,
}

fn process_batch<F>(
    inputs: &[PathBuf],
    chosen_output: Option<&Path>,
    force_cpu: bool,
    mut on_event: F,
) -> Result<BatchSummary, Box<dyn std::error::Error>>
where
    F: FnMut(ProcessingEvent),
{
    // One model lifecycle for the complete request, regardless of caller.
    on_event(ProcessingEvent::LoadingModel);
    let mut session = create_session(force_cpu)?;

    // The authoritative list is prepared after the model is started.
    let files = prepare_input_list(inputs);
    if files.is_empty() {
        on_event(ProcessingEvent::UnloadingModel);
        drop(session);
        return Err("No supported image files found".into());
    }

    let input_name = session
        .inputs()
        .first()
        .map(|o| o.name().to_string())
        .ok_or("The ONNX model does not report an input")?;
    let fixed_hw = model_input_hw(&session);

    let total = files.len();
    let batches = (total + 1) / 2;
    on_event(ProcessingEvent::ListReady { total, batches });

    let mut summary = BatchSummary {
        total,
        ..BatchSummary::default()
    };
    let mut completed = 0usize;

    // Queue rule: at most two images belong to a queue group. The model itself
    // remains a single Session, so inference stays stable and VRAM use does not
    // multiply. An odd final image forms a one-image group.
    for (batch_index, pair) in files.chunks(2).enumerate() {
        for file in pair {
            let index = completed + 1;
            on_event(ProcessingEvent::ImageStarted {
                index,
                total,
                batch: batch_index + 1,
                batches,
                path: file.clone(),
            });

            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                let output_dir = output_directory_for(file, chosen_output);
                std::fs::create_dir_all(&output_dir)?;
                process_image(file, &output_dir, &mut session, &input_name, fixed_hw)
            })();

            match result {
                Ok(()) => summary.succeeded += 1,
                Err(err) => summary.failures.push(BatchFailure {
                    path: file.clone(),
                    error: err.to_string(),
                }),
            }

            completed += 1;
            on_event(ProcessingEvent::ImageFinished {
                index: completed,
                total,
                succeeded: summary.succeeded,
                failed: summary.failures.len(),
            });
        }
    }

    on_event(ProcessingEvent::UnloadingModel);
    drop(session);
    Ok(summary)
}

fn run_cli(
    args: &Args,
    inputs: &[PathBuf],
) -> Result<BatchSummary, Box<dyn std::error::Error>> {
    let _requested_threads = args.threads.max(1);

    let summary = process_batch(
        inputs,
        args.output_dir.as_deref(),
        args.cpu,
        |event| match event {
            ProcessingEvent::LoadingModel => println!("Loading AI model..."),
            ProcessingEvent::ListReady { total, batches } => {
                println!("Prepared {total} image(s) in {batches} queue group(s), maximum 2 per group.");
            }
            ProcessingEvent::ImageStarted {
                index,
                total,
                batch,
                batches,
                path,
            } => {
                println!(
                    "Processing {index}/{total} (queue {batch}/{batches}): {}",
                    path.display()
                );
            }
            ProcessingEvent::ImageFinished {
                index,
                total,
                succeeded,
                failed,
            } => {
                println!("Progress {index}/{total} - OK: {succeeded}, failed: {failed}");
            }
            ProcessingEvent::UnloadingModel => println!("Unloading AI model..."),
        },
    )?;

    println!(
        "Done. {} succeeded, {} failed.",
        summary.succeeded,
        summary.failures.len()
    );
    for failure in &summary.failures {
        eprintln!("Failed: {} - {}", failure.path.display(), failure.error);
    }

    Ok(summary)
}

fn run_gui(force_cpu: bool) -> Result<(), Box<dyn std::error::Error>> {
    use slint::winit_030::{winit, EventResult, WinitWindowAccessor};

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .select()?;

    let ui = MainWindow::new()?;
    ui.set_model_status(model_status_text().into());

    let files_state: Rc<RefCell<Vec<PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    let output_state: Rc<RefCell<Option<PathBuf>>> = Rc::new(RefCell::new(None));

    {
        let weak = ui.as_weak();
        let files_state = files_state.clone();
        ui.on_add_files(move || {
            let Some(paths) = rfd::FileDialog::new()
                .add_filter("Images", &["jpg", "jpeg", "png", "webp", "avif", "heic", "heif"])
                .pick_files()
            else {
                return;
            };

            let mut files = files_state.borrow_mut();
            for path in paths {
                add_path_to_list(&mut files, &path);
            }

            if let Some(ui) = weak.upgrade() {
                sync_gui_file_state(&ui, &files);
                ui.set_status_text("Ready".into());
            }
        });
    }

    {
        let weak = ui.as_weak();
        let files_state = files_state.clone();
        ui.on_add_folder(move || {
            let Some(folder) = rfd::FileDialog::new().pick_folder() else {
                return;
            };
            let mut files = files_state.borrow_mut();
            add_path_to_list(&mut files, &folder);
            if let Some(ui) = weak.upgrade() {
                sync_gui_file_state(&ui, &files);
                ui.set_status_text("Folder scanned for supported images.".into());
            }
        });
    }

    {
        let weak = ui.as_weak();
        let files_state = files_state.clone();
        ui.on_clear_files(move || {
            files_state.borrow_mut().clear();
            if let Some(ui) = weak.upgrade() {
                sync_gui_file_state(&ui, &[]);
                ui.set_progress_value(0);
                ui.set_status_text("Ready".into());
            }
        });
    }

    {
        let weak = ui.as_weak();
        let output_state = output_state.clone();
        ui.on_choose_output(move || {
            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                *output_state.borrow_mut() = Some(folder.clone());
                if let Some(ui) = weak.upgrade() {
                    ui.set_output_dir(folder.display().to_string().into());
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        ui.on_install_context_menu(move || {
            let result = register_context_menu();
            if let Some(ui) = weak.upgrade() {
                match result {
                    Ok(()) => ui.set_status_text(
                        "Explorer menu installed: Actions > Remove background".into(),
                    ),
                    Err(err) => ui.set_status_text(format!("Context menu error: {err}").into()),
                }
            }
        });
    }

    {
        let weak = ui.as_weak();
        let files_state = files_state.clone();
        let output_state = output_state.clone();
        ui.on_start_processing(move || {
            let Some(ui_now) = weak.upgrade() else {
                return;
            };
            if ui_now.get_busy() {
                return;
            }

            let files = files_state.borrow().clone();
            if files.is_empty() {
                ui_now.set_status_text("Add at least one supported image.".into());
                return;
            }

            let output_dir = output_state.borrow().clone();
            ui_now.set_busy(true);
            ui_now.set_progress_value(0);
            ui_now.set_status_text("Loading AI model...".into());

            let weak_worker = weak.clone();
            std::thread::spawn(move || {
                let worker_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    process_batch(
                        &files,
                        output_dir.as_deref(),
                        force_cpu,
                        |event| {
                            let weak_progress = weak_worker.clone();
                            match event {
                                ProcessingEvent::LoadingModel => {
                                    weak_progress
                                        .upgrade_in_event_loop(|ui| {
                                            ui.set_status_text("Loading AI model...".into());
                                        })
                                        .ok();
                                }
                                ProcessingEvent::ListReady { total, batches } => {
                                    weak_progress
                                        .upgrade_in_event_loop(move |ui| {
                                            ui.set_status_text(
                                                format!(
                                                    "Prepared {total} image(s) in {batches} queue group(s)."
                                                )
                                                .into(),
                                            );
                                        })
                                        .ok();
                                }
                                ProcessingEvent::ImageStarted {
                                    index,
                                    total,
                                    batch,
                                    batches,
                                    path,
                                } => {
                                    let display_name = path
                                        .file_name()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("image")
                                        .to_string();
                                    let progress_before =
                                        (((index.saturating_sub(1)) * 100) / total.max(1)) as i32;
                                    weak_progress
                                        .upgrade_in_event_loop(move |ui| {
                                            ui.set_progress_value(progress_before);
                                            ui.set_status_text(
                                                format!(
                                                    "Queue {batch}/{batches} · {index}/{total}: {display_name}"
                                                )
                                                .into(),
                                            );
                                        })
                                        .ok();
                                }
                                ProcessingEvent::ImageFinished {
                                    index,
                                    total,
                                    succeeded,
                                    failed,
                                } => {
                                    let progress = ((index * 100) / total.max(1)) as i32;
                                    weak_progress
                                        .upgrade_in_event_loop(move |ui| {
                                            ui.set_progress_value(progress);
                                            ui.set_status_text(
                                                format!(
                                                    "Processed {index}/{total} · OK {succeeded} · Failed {failed}"
                                                )
                                                .into(),
                                            );
                                        })
                                        .ok();
                                }
                                ProcessingEvent::UnloadingModel => {
                                    weak_progress
                                        .upgrade_in_event_loop(|ui| {
                                            ui.set_status_text(
                                                "Finishing queue and unloading AI model...".into(),
                                            );
                                        })
                                        .ok();
                                }
                            }
                        },
                    )
                    .map_err(|e| e.to_string())
                }));

                let result: Result<BatchSummary, String> = match worker_result {
                    Ok(result) => result,
                    Err(_) => Err("The processing worker stopped unexpectedly. The UI remains usable; try the batch again.".to_string()),
                };

                let weak_done = weak_worker.clone();
                weak_done
                    .upgrade_in_event_loop(move |ui| {
                        ui.set_busy(false);
                        match result {
                            Ok(summary) if summary.failures.is_empty() => {
                                ui.set_progress_value(100);
                                ui.set_status_text(
                                    format!(
                                        "Finished. {} transparent PNG output(s) saved.",
                                        summary.succeeded
                                    )
                                    .into(),
                                );
                            }
                            Ok(summary) => {
                                ui.set_progress_value(100);
                                let first = summary
                                    .failures
                                    .first()
                                    .map(|f| format!(" First error: {}", f.error))
                                    .unwrap_or_default();
                                ui.set_status_text(
                                    format!(
                                        "Finished: {} succeeded, {} failed.{}",
                                        summary.succeeded,
                                        summary.failures.len(),
                                        first
                                    )
                                    .into(),
                                );
                            }
                            Err(err) => {
                                ui.set_status_text(format!("Error: {err}").into());
                            }
                        }
                    })
                    .ok();
            });
        });
    }

    {
        let weak = ui.as_weak();
        let files_state = files_state.clone();
        ui.window().on_winit_window_event(move |_window, event| {
            match event {
                winit::event::WindowEvent::HoveredFile(path) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_drag_active(path.is_dir() || is_supported_image(&path));
                    }
                }
                winit::event::WindowEvent::HoveredFileCancelled => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_drag_active(false);
                    }
                }
                winit::event::WindowEvent::DroppedFile(path) => {
                    if let Some(ui) = weak.upgrade() {
                        ui.set_drag_active(false);
                    }

                    if path.is_dir() || is_supported_image(&path) {
                        let mut files = files_state.borrow_mut();
                        add_path_to_list(&mut files, &path);
                        if let Some(ui) = weak.upgrade() {
                            sync_gui_file_state(&ui, &files);
                            ui.set_status_text("Image(s) added. Press Remove background and save.".into());
                        }
                    } else if let Some(ui) = weak.upgrade() {
                        ui.set_status_text("Unsupported file. Use JPG, PNG, WEBP, AVIF, HEIC or HEIF.".into());
                    }
                }
                _ => {}
            }
            EventResult::Propagate
        });
    }

    ui.run()?;
    Ok(())
}


#[cfg(target_os = "windows")]
struct ContextBatchLock(*mut std::ffi::c_void);

#[cfg(target_os = "windows")]
impl Drop for ContextBatchLock {
    fn drop(&mut self) {
        unsafe {
            let _ = ReleaseMutex(self.0);
            let _ = CloseHandle(self.0);
        }
    }
}

struct ContextBatchLease {
    inputs: Vec<PathBuf>,
    #[cfg(target_os = "windows")]
    _lock: ContextBatchLock,
}

#[cfg(target_os = "windows")]
fn context_queue_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Gallery Inc")
        .join("Background Remover AI")
        .join("context-queue")
}

#[cfg(target_os = "windows")]
fn cleanup_stale_context_requests() {
    use std::time::{Duration, SystemTime};

    let Ok(entries) = std::fs::read_dir(context_queue_dir()) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !matches!(path.extension().and_then(|s| s.to_str()), Some("req" | "tmp")) {
            continue;
        }
        let stale = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .map(|age| age > Duration::from_secs(300))
            .unwrap_or(false);
        if stale {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[cfg(target_os = "windows")]
fn enqueue_context_inputs(inputs: &[PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::windows::ffi::OsStrExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    let dir = context_queue_dir();
    std::fs::create_dir_all(&dir)?;
    cleanup_stale_context_requests();
    let pid = std::process::id();
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();

    for (index, input) in inputs.iter().enumerate() {
        let request_path = dir.join(format!("{pid}-{stamp}-{index}.req"));
        let temp_path = dir.join(format!("{pid}-{stamp}-{index}.tmp"));
        let mut bytes = Vec::new();
        for unit in input.as_os_str().encode_wide() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        std::fs::write(&temp_path, bytes)?;
        std::fs::rename(&temp_path, &request_path)?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn acquire_context_batch_lock() -> Result<Option<ContextBatchLock>, Box<dyn std::error::Error>> {
    use std::os::windows::ffi::OsStrExt;

    const WAIT_OBJECT_0: u32 = 0;
    const WAIT_TIMEOUT: u32 = 258;

    let name: Vec<u16> = std::ffi::OsStr::new(
        r"Local\GalleryInc.BackgroundRemoverAI.ContextBatch",
    )
    .encode_wide()
    .chain(std::iter::once(0))
    .collect();

    let handle = unsafe { CreateMutexW(std::ptr::null(), 0, name.as_ptr()) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }

    match unsafe { WaitForSingleObject(handle, 0) } {
        WAIT_OBJECT_0 => Ok(Some(ContextBatchLock(handle))),
        WAIT_TIMEOUT => {
            unsafe {
                let _ = CloseHandle(handle);
            }
            Ok(None)
        }
        _ => {
            unsafe {
                let _ = CloseHandle(handle);
            }
            Err(std::io::Error::last_os_error().into())
        }
    }
}

#[cfg(target_os = "windows")]
fn read_context_request(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    use std::os::windows::ffi::OsStringExt;

    let bytes = std::fs::read(path)?;
    if bytes.len() % 2 != 0 {
        return Err("Invalid Explorer batch request".into());
    }
    let mut wide = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        wide.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&wide)))
}

#[cfg(target_os = "windows")]
fn pending_context_request_count() -> usize {
    let Ok(entries) = std::fs::read_dir(context_queue_dir()) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("req"))
        .count()
}

#[cfg(target_os = "windows")]
fn drain_context_requests() -> Vec<PathBuf> {
    let dir = context_queue_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut requests = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("req") {
            continue;
        }
        if let Ok(input) = read_context_request(&path) {
            requests.push(input);
        }
        let _ = std::fs::remove_file(path);
    }
    requests.sort();
    requests.dedup();
    requests
}

#[cfg(target_os = "windows")]
fn collect_context_batch(
    inputs: &[PathBuf],
) -> Result<Option<ContextBatchLease>, Box<dyn std::error::Error>> {
    use std::time::{Duration, Instant};

    enqueue_context_inputs(inputs)?;
    let Some(lock) = acquire_context_batch_lock()? else {
        // Another instance owns the batch and will collect this request.
        return Ok(None);
    };

    // Explorer may start one process per selected item. Wait for a short quiet
    // period so those invocations collapse into one model lifecycle.
    let started = Instant::now();
    let mut last_count = pending_context_request_count();
    let mut stable_since = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(75));
        let count = pending_context_request_count();
        if count != last_count {
            last_count = count;
            stable_since = Instant::now();
        }
        if stable_since.elapsed() >= Duration::from_millis(350)
            || started.elapsed() >= Duration::from_millis(1500)
        {
            break;
        }
    }

    let collected = drain_context_requests();
    Ok(Some(ContextBatchLease {
        inputs: collected,
        _lock: lock,
    }))
}

#[cfg(not(target_os = "windows"))]
fn collect_context_batch(
    inputs: &[PathBuf],
) -> Result<Option<ContextBatchLease>, Box<dyn std::error::Error>> {
    Ok(Some(ContextBatchLease {
        inputs: inputs.to_vec(),
    }))
}

#[cfg(target_os = "windows")]
fn drain_late_context_requests() -> Vec<PathBuf> {
    if pending_context_request_count() == 0 {
        return Vec::new();
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
    drain_context_requests()
}

#[cfg(not(target_os = "windows"))]
fn drain_late_context_requests() -> Vec<PathBuf> {
    Vec::new()
}

fn show_context_summary(summary: &BatchSummary) {
    if summary.failures.is_empty() {
        return;
    }

    let mut details = format!(
        "Processed {} image(s): {} succeeded, {} failed.",
        summary.total,
        summary.succeeded,
        summary.failures.len()
    );
    for failure in summary.failures.iter().take(5) {
        details.push_str(&format!(
            "\n\n{}\n{}",
            failure.path.display(),
            failure.error
        ));
    }
    if summary.failures.len() > 5 {
        details.push_str("\n\nAdditional failed images were omitted from this dialog.");
    }

    let _ = rfd::MessageDialog::new()
        .set_title("Background Remover AI")
        .set_description(details)
        .set_level(rfd::MessageLevel::Warning)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let terminal_mode = !args.context_menu
        && (args.add || args.remove_context_menu || !args.input.is_empty());
    if terminal_mode {
        attach_parent_console();
    }

    if !args.input.is_empty() {
        configure_process_priority();
    }

    if args.add {
        register_context_menu()?;
        println!("Explorer Actions menu installed for supported images and folders.");
        return Ok(());
    }

    if args.remove_context_menu {
        unregister_context_menu()?;
        println!("Explorer Actions menu removed.");
        return Ok(());
    }

    if args.context_menu {
        if args.input.is_empty() {
            return Ok(());
        }

        let Some(mut lease) = collect_context_batch(&args.input)? else {
            return Ok(());
        };

        loop {
            if !lease.inputs.is_empty() {
                match run_cli(&args, &lease.inputs) {
                    Ok(summary) => show_context_summary(&summary),
                    Err(err) => {
                        let _ = rfd::MessageDialog::new()
                            .set_title("Background Remover AI")
                            .set_description(format!("Explorer Action failed:\n\n{err}"))
                            .set_level(rfd::MessageLevel::Error)
                            .set_buttons(rfd::MessageButtons::Ok)
                            .show();
                    }
                }
            }

            let late = drain_late_context_requests();
            if late.is_empty() {
                break;
            }
            lease.inputs = late;
        }
        return Ok(());
    }

    if !args.input.is_empty() {
        let summary = run_cli(&args, &args.input)?;
        if summary.succeeded == 0 && !summary.failures.is_empty() {
            return Err("All images in the batch failed".into());
        }
        return Ok(());
    }

    run_gui(args.cpu)
}

fn model_input_hw(session: &Session) -> Option<(u32, u32)> {
    let outlet = session.inputs().first()?;
    if let ValueType::Tensor { shape, .. } = outlet.dtype() {
        if shape.len() >= 2 {
            let h = shape[shape.len() - 2];
            let w = shape[shape.len() - 1];
            if h > 0 && w > 0 {
                return Some((h as u32, w as u32));
            }
        }
    }
    None
}

fn process_image(
    input_path: &Path,
    output_dir: &Path,
    session: &mut Session,
    input_name: &str,
    fixed_hw: Option<(u32, u32)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open(input_path)?;
    let (orig_width, orig_height) = img.dimensions();

    let (target_h, target_w) = match fixed_hw {
        Some((h, w)) => (h, w),
        None => (
            ((orig_height + 31) / 32) * 32,
            ((orig_width + 31) / 32) * 32,
        ),
    };

    let input_data = preprocess_image(&img, target_w, target_h)?;
    let shape = [1i64, 3, target_h as i64, target_w as i64];
    let input_val = Value::from_array((shape, input_data))?;

    let (dims, out_data) = {
        let outputs = session.run(inputs![input_name => input_val])?;
        let output_value = outputs.iter().next().ok_or("ONNX model returned no output")?.1;
        let (out_shape, out_tensor_data) = output_value.try_extract_tensor::<f32>()?;

        let dims: Vec<i64> = out_shape.iter().copied().collect();
        (dims, out_tensor_data.to_vec())
    };

    let mask_image = postprocess_mask(&dims, &out_data, target_w, target_h)?;
    let mask_resized = image::imageops::resize(
        &mask_image,
        orig_width,
        orig_height,
        image::imageops::FilterType::Triangle,
    );

    let mut final_img = img.to_rgba8();
    for (x, y, pixel) in final_img.enumerate_pixels_mut() {
        let mask_pixel = mask_resized.get_pixel(x, y);
        pixel[3] = mask_pixel[0];
    }

    let file_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let output_file = output_dir.join(format!("{}_nobg.png", file_stem));
    final_img.save_with_format(output_file, image::ImageFormat::Png)?;

    Ok(())
}

fn preprocess_image(
    img: &DynamicImage,
    target_w: u32,
    target_h: u32,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let resized = img.resize_exact(target_w, target_h, image::imageops::FilterType::Triangle);
    let rgb_img = resized.to_rgb8();

    let mean = [0.485f32, 0.456, 0.406];
    let std = [0.229f32, 0.224, 0.225];
    let (w, h) = (target_w as usize, target_h as usize);
    let mut data = vec![0f32; 3 * w * h];

    for (x, y, pixel) in rgb_img.enumerate_pixels() {
        let (x, y) = (x as usize, y as usize);
        for c in 0..3 {
            data[c * w * h + y * w + x] =
                ((pixel[c] as f32 / 255.0) - mean[c]) / std[c];
        }
    }
    Ok(data)
}

fn postprocess_mask(
    dims: &[i64],
    data: &[f32],
    width: u32,
    height: u32,
) -> Result<image::GrayImage, Box<dyn std::error::Error>> {
    let (w, h) = (width as usize, height as usize);
    let (mask_h, mask_w) = if dims.len() >= 2 {
        (
            dims[dims.len() - 2] as usize,
            dims[dims.len() - 1] as usize,
        )
    } else {
        (h, w)
    };

    if mask_h * mask_w > data.len() {
        return Err(format!("Mask shape {:?} does not match output size", dims).into());
    }

    let mut mask = image::GrayImage::new(width, height);
    for y in 0..h.min(mask_h) {
        for x in 0..w.min(mask_w) {
            let val = data[y * mask_w + x];
            let scaled_val = (val.clamp(0.0, 1.0) * 255.0) as u8;
            mask.put_pixel(x as u32, y as u32, image::Luma([scaled_val]));
        }
    }
    Ok(mask)
}

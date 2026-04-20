use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv",
];
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "aac", "ogg", "opus", "m4a", "aiff", "wma",
];
/// All media extensions recognised by the app (video + audio).
const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv",
    "mp3", "flac", "wav", "aac", "ogg", "opus", "m4a", "aiff", "wma",
];
#[cfg(any(target_os = "macos", target_os = "ios"))]
use tauri::RunEvent;
use tauri::{Emitter, Manager};
use tauri::{PhysicalPosition, PhysicalSize};

// Helper to create a Command with hidden console window on Windows
fn create_hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd
}

// Global state to store pending file paths
static PENDING_FILES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static FILE_PROCESSED: Mutex<bool> = Mutex::new(false);

// PiP window state storage
#[derive(Clone, Debug)]
struct WindowState {
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    decorations_enabled: bool,
    resizable: bool,
    pip_active: bool,
}

static WINDOW_STATE: Mutex<Option<WindowState>> = Mutex::new(None);

// Configuration constants
const MAX_FILE_LOADING_ATTEMPTS: u32 = 30;
const FRONTEND_READY_WAIT_MS: u64 = 500;
const INITIAL_ATTEMPT_DELAY_MS: u64 = 2000;
const MIDDLE_ATTEMPT_DELAY_MS: u64 = 1000;
const FINAL_ATTEMPT_DELAY_MS: u64 = 500;
const INITIAL_ATTEMPT_COUNT: u32 = 5;
const MIDDLE_ATTEMPT_COUNT: u32 = 15;

// PiP window configuration from constants.json
#[derive(Deserialize)]
struct PipWindowConfig {
    width: u32,
    height: u32,
    padding: i32,
}

#[derive(Deserialize)]
struct AppConstants {
    #[serde(rename = "pipWindow")]
    pip_window: PipWindowConfig,
}

fn get_pip_constants() -> Result<PipWindowConfig, anyhow::Error> {
    const CONSTANTS_JSON: &str = include_str!("../../constants.json");
    let constants: AppConstants = serde_json::from_str(CONSTANTS_JSON)
        .map_err(|e| anyhow!("Failed to parse constants.json: {}", e))?;
    Ok(constants.pip_window)
}

// Path sanitization function
fn sanitize_path(path: &str) -> String {
    let mut clean_path = path.trim().to_string();

    // Remove surrounding quotes if present
    if clean_path.starts_with('"') && clean_path.ends_with('"') {
        clean_path = clean_path[1..clean_path.len() - 1].to_string();
    }

    // Handle Windows UNC paths and long path names
    if clean_path.starts_with("\\\\?\\") {
        clean_path = clean_path[4..].to_string();
    }

    clean_path
}

#[tauri::command]
async fn open_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file_path = app
        .dialog()
        .file()
        .add_filter("Video Files", VIDEO_EXTENSIONS)
        .add_filter("Audio Files", AUDIO_EXTENSIONS)
        .blocking_pick_file();

    match file_path {
        Some(file) => {
            let path_buf = file.into_path().map_err(|e| e.to_string())?;
            let path = path_buf.to_string_lossy().to_string();
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

#[tauri::command]
async fn open_subtitle_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let file_path = app
        .dialog()
        .file()
        .add_filter("Subtitle Files", &["srt", "vtt", "ass", "ssa", "sub"])
        .blocking_pick_file();

    match file_path {
        Some(file) => {
            let path_buf = file.into_path().map_err(|e| e.to_string())?;
            let path = path_buf.to_string_lossy().to_string();
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

#[tauri::command]
fn convert_file_path(path: String) -> Result<String, String> {
    Ok(format!("https://asset.localhost/{}", path))
}

#[tauri::command]
fn get_pending_file() -> Option<String> {
    let mut pending = PENDING_FILES.lock().unwrap();
    pending.pop_front()
}

#[tauri::command]
fn mark_file_processed() {
    let mut processed = FILE_PROCESSED.lock().unwrap();
    *processed = true;
}

#[tauri::command]
fn frontend_ready(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(debug_assertions)]
    println!("Frontend is ready to receive events");

    // Check if there are any pending files and emit them now
    let pending_files: Vec<String> = {
        let mut pending = PENDING_FILES.lock().unwrap();
        pending.drain(..).collect()
    };

    if !pending_files.is_empty() {
        #[cfg(debug_assertions)]
        println!(
            "Frontend ready - processing {} pending files",
            pending_files.len()
        );
        process_video_files(&app_handle, pending_files);
    }

    Ok(())
}

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    #[cfg(debug_assertions)]
    println!("Exit app command called");
    app_handle.exit(0);
}

#[tauri::command]
fn enter_pip_mode(app_handle: tauri::AppHandle) -> Result<(), String> {
    let window = app_handle
        .get_webview_window("main")
        .ok_or("Failed to get main window")?;

    // Check if already in PiP mode (idempotency)
    {
        let state = WINDOW_STATE.lock().unwrap();
        if let Some(saved_state) = state.as_ref() {
            if saved_state.pip_active {
                #[cfg(debug_assertions)]
                println!("Already in PiP mode, skipping");
                return Ok(());
            }
        }
    }

    // Get PiP configuration from constants.json
    let pip_config =
        get_pip_constants().map_err(|e| format!("Failed to load PiP configuration: {}", e))?;

    // Save current window state
    let current_size = window
        .outer_size()
        .map_err(|e| format!("Failed to get window size: {}", e))?;
    let current_position = window
        .outer_position()
        .map_err(|e| format!("Failed to get window position: {}", e))?;
    let current_decorations = window
        .is_decorated()
        .map_err(|e| format!("Failed to get decoration state: {}", e))?;
    let current_resizable = window
        .is_resizable()
        .map_err(|e| format!("Failed to get resizable state: {}", e))?;

    let mut state = WINDOW_STATE.lock().unwrap();
    *state = Some(WindowState {
        size: current_size,
        position: current_position,
        decorations_enabled: current_decorations,
        resizable: current_resizable,
        pip_active: true,
    });

    #[cfg(debug_assertions)]
    println!("Saved window state: {:?}", state);

    // Enable decorations for PiP mode so window can be dragged
    window
        .set_decorations(true)
        .map_err(|e| format!("Failed to enable decorations: {}", e))?;

    // Enable resizing for PiP mode
    window
        .set_resizable(true)
        .map_err(|e| format!("Failed to enable resizing: {}", e))?;

    // Set PiP window properties
    window
        .set_size(PhysicalSize::new(pip_config.width, pip_config.height))
        .map_err(|e| format!("Failed to set window size: {}", e))?;

    // Position at bottom-right corner with some padding
    // Get monitor size or use fallback defaults
    let (screen_width, screen_height) = if let Ok(Some(monitor)) = window.current_monitor() {
        let size = monitor.size();
        (size.width as i32, size.height as i32)
    } else {
        // Fallback when monitor detection fails
        (1920i32, 1080i32)
    };

    // Calculate position from screen size
    let x = screen_width - (pip_config.width as i32) - pip_config.padding;
    let y = screen_height - (pip_config.height as i32) - pip_config.padding;
    let position = PhysicalPosition::new(x, y);

    window
        .set_position(position)
        .map_err(|e| format!("Failed to set window position: {}", e))?;

    // Set window to always on top
    window
        .set_always_on_top(true)
        .map_err(|e| format!("Failed to set always on top: {}", e))?;

    #[cfg(debug_assertions)]
    println!("Entered PiP mode");

    Ok(())
}

#[tauri::command]
fn exit_pip_mode(app_handle: tauri::AppHandle) -> Result<(), String> {
    let window = app_handle
        .get_webview_window("main")
        .ok_or("Failed to get main window")?;

    // Validate that we are in PiP mode and have saved state
    let mut state = WINDOW_STATE.lock().unwrap();
    let saved_state = state
        .take()
        .ok_or("Cannot exit PiP mode: no saved window state found")?;

    if !saved_state.pip_active {
        return Err("Cannot exit PiP mode: PiP mode is not currently active".to_string());
    }

    #[cfg(debug_assertions)]
    println!("Restoring window state: {:?}", saved_state);

    // Restore size and position
    window
        .set_size(saved_state.size)
        .map_err(|e| format!("Failed to restore window size: {}", e))?;
    window
        .set_position(saved_state.position)
        .map_err(|e| format!("Failed to restore window position: {}", e))?;

    // Restore original decoration state
    window
        .set_decorations(saved_state.decorations_enabled)
        .map_err(|e| format!("Failed to restore decoration state: {}", e))?;

    // Restore original resizable state
    window
        .set_resizable(saved_state.resizable)
        .map_err(|e| format!("Failed to restore resizable state: {}", e))?;

    // Remove always on top
    window
        .set_always_on_top(false)
        .map_err(|e| format!("Failed to remove always on top: {}", e))?;

    #[cfg(debug_assertions)]
    println!("Exited PiP mode");

    Ok(())
}

#[tauri::command]
fn find_subtitle_for_video(video_path: String) -> Result<Option<String>, String> {
    use std::path::Path;

    let video_path_obj = Path::new(&video_path);
    let video_dir = video_path_obj
        .parent()
        .ok_or("Could not get video directory")?;
    let video_stem = video_path_obj
        .file_stem()
        .ok_or("Could not get video filename")?;
    let video_stem_str = video_stem.to_string_lossy();
    let video_stem_lower = video_stem_str.to_lowercase();

    // Subtitle extensions to check
    let subtitle_exts = vec!["srt", "vtt", "ass", "ssa", "sub"];

    // First try: exact case match (fastest, works on case-insensitive filesystems)
    for ext in &subtitle_exts {
        let subtitle_path = video_dir.join(format!("{}.{}", video_stem_str, ext));
        if subtitle_path.exists() {
            #[cfg(debug_assertions)]
            println!("Found subtitle file (exact match): {:?}", subtitle_path);
            return Ok(Some(subtitle_path.to_string_lossy().to_string()));
        }
    }

    // Second try: case-insensitive search (for case-sensitive filesystems)
    // Read directory entries and match case-insensitively
    if let Ok(entries) = fs::read_dir(video_dir) {
        for entry in entries.flatten() {
            let entry_path = entry.path();

            // Get the file stem and extension
            if let (Some(file_stem), Some(file_ext)) = (
                entry_path.file_stem().and_then(|s| s.to_str()),
                entry_path.extension().and_then(|s| s.to_str()),
            ) {
                let file_stem_lower = file_stem.to_lowercase();
                let file_ext_lower = file_ext.to_lowercase();

                // Check if stem matches (case-insensitive) and extension is a subtitle format
                if file_stem_lower == video_stem_lower
                    && subtitle_exts.contains(&file_ext_lower.as_str())
                {
                    #[cfg(debug_assertions)]
                    println!(
                        "Found subtitle file (case-insensitive match): {:?}",
                        entry_path
                    );
                    return Ok(Some(entry_path.to_string_lossy().to_string()));
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    println!("No subtitle file found for video: {}", video_path);
    Ok(None)
}

// Return the list of text-based subtitle streams embedded in a video file.
// Bitmap formats (PGS, VobSub, DVB) are silently skipped because they cannot
// be converted to a text format that the browser can render.
#[tauri::command]
async fn get_embedded_subtitle_tracks(
    video_path: String,
) -> Result<Vec<EmbeddedSubtitleTrack>, String> {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    let task = tokio::task::spawn_blocking(move || {
        const SUPPORTED: &[&str] =
            &["subrip", "ass", "ssa", "webvtt", "mov_text", "srt", "text"];

        let output = create_hidden_command("ffprobe")
            .args([
                "-v",
                "error",
                "-select_streams",
                "s",
                "-show_entries",
                "stream=index,codec_name:stream_tags=language,title",
                "-of",
                "json",
                &video_path,
            ])
            .output()
            .map_err(|e| format!("Failed to run ffprobe: {}", e))?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
            Ok(v) => v,
            Err(_) => return Ok(vec![]),
        };

        let streams = match parsed["streams"].as_array() {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        let mut tracks = Vec::new();
        for stream in streams {
            let codec_name = stream["codec_name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string();

            if !SUPPORTED.contains(&codec_name.as_str()) {
                continue;
            }

            let Some(index) = stream["index"].as_i64() else {
                continue;
            };
            tracks.push(EmbeddedSubtitleTrack {
                index,
                codec_name,
                language: stream["tags"]["language"].as_str().map(|s| s.to_string()),
                title: stream["tags"]["title"].as_str().map(|s| s.to_string()),
            });
        }

        #[cfg(debug_assertions)]
        println!(
            "Found {} embedded subtitle track(s) in: {}",
            tracks.len(),
            video_path
        );

        Ok::<Vec<EmbeddedSubtitleTrack>, String>(tracks)
    });

    tokio::time::timeout(TIMEOUT, task)
        .await
        .map_err(|_| "ffprobe timed out after 30 seconds".to_string())?
        .map_err(|e| format!("Task panicked: {}", e))?
}

// Extract a single subtitle stream from a video file and return its content as
// an SRT string. FFmpeg handles codec conversion (e.g. ASS → SRT) automatically
// when the output format is forced to `srt`.  Sending output to `pipe:1` means
// no temp file is written to disk.
#[tauri::command]
async fn extract_embedded_subtitle(
    video_path: String,
    stream_index: i64,
) -> Result<String, String> {
    if stream_index < 0 {
        return Err(format!("Invalid stream index: {}", stream_index));
    }

    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    let task = tokio::task::spawn_blocking(move || {
        #[cfg(debug_assertions)]
        println!(
            "Extracting embedded subtitle stream {} from: {}",
            stream_index, video_path
        );

        let output = create_hidden_command("ffmpeg")
            .args([
                "-v",
                "error",
                "-i",
                &video_path,
                "-map",
                &format!("0:{}", stream_index),
                "-f",
                "srt",
                "pipe:1",
            ])
            .output()
            .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("FFmpeg failed to extract subtitle: {}", stderr));
        }

        Ok::<String, String>(String::from_utf8_lossy(&output.stdout).to_string())
    });

    tokio::time::timeout(TIMEOUT, task)
        .await
        .map_err(|_| "ffmpeg timed out after 30 seconds".to_string())?
        .map_err(|e| format!("Task panicked: {}", e))?
}

#[derive(Serialize, Clone)]
struct VideoFile {
    path: String,
    name: String,
    size: u64,
    modified: u64,
    duration: Option<f64>,
}

#[derive(Serialize, serde::Deserialize, Clone)]
struct WatchProgress {
    path: String,
    current_time: f64,
    duration: f64,
    last_watched: u64,
}

#[derive(Serialize, Clone)]
struct ConversionProgress {
    stage: String,
    progress: f32,
    message: String,
}

#[derive(Serialize, Clone)]
struct VideoInfo {
    format: String,
    size_mb: f64,
}

#[derive(Serialize, Clone)]
struct EmbeddedSubtitleTrack {
    index: i64,
    codec_name: String,
    language: Option<String>,
    title: Option<String>,
}

// Get video duration using FFmpeg
// Note: Caller should verify ffprobe is available before calling this
fn get_video_duration(video_path: &str) -> Option<f64> {
    let output = create_hidden_command("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            video_path,
        ])
        .output()
        .ok()?;

    if output.status.success() {
        let duration_str = String::from_utf8_lossy(&output.stdout);
        duration_str.trim().parse::<f64>().ok()
    } else {
        None
    }
}

#[tauri::command]
fn save_watch_progress(video_path: String, current_time: f64, duration: f64) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let progress_file = config_dir.join("watch_progress.json");

    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    // Load existing progress data or create new
    let mut progress_map: std::collections::HashMap<String, WatchProgress> =
        if progress_file.exists() {
            let content = fs::read_to_string(&progress_file)
                .map_err(|e| format!("Failed to read progress file: {}", e))?;
            serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new())
        } else {
            std::collections::HashMap::new()
        };

    // Get current time as Unix timestamp
    let last_watched = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get current time: {}", e))?
        .as_secs();

    // Update or insert progress for this video
    progress_map.insert(
        video_path.clone(),
        WatchProgress {
            path: video_path,
            current_time,
            duration,
            last_watched,
        },
    );

    // Save to file
    let content = serde_json::to_string_pretty(&progress_map)
        .map_err(|e| format!("Failed to serialize progress: {}", e))?;

    fs::write(progress_file, content)
        .map_err(|e| format!("Failed to write progress file: {}", e))?;

    Ok(())
}

// Get watch progress for a video
#[tauri::command]
fn get_watch_progress(video_path: String) -> Result<Option<WatchProgress>, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let progress_file = config_dir.join("watch_progress.json");

    if !progress_file.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&progress_file)
        .map_err(|e| format!("Failed to read progress file: {}", e))?;

    let progress_map: std::collections::HashMap<String, WatchProgress> =
        serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new());

    Ok(progress_map.get(&video_path).cloned())
}

// Get video file info
#[tauri::command]
fn get_video_info(video_path: String) -> Result<VideoInfo, String> {
    let path = Path::new(&video_path);

    // Get file size
    let metadata = fs::metadata(path).map_err(|e| format!("Failed to get file metadata: {}", e))?;
    let size_bytes = metadata.len();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

    // Get format from extension
    let format = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("unknown")
        .to_uppercase();

    Ok(VideoInfo { format, size_mb })
}

// Estimate converted file size (rough estimation)
#[allow(dead_code)]
fn estimate_converted_size(original_size_mb: f64, target_format: &str) -> f64 {
    // Simple compression ratio estimates
    match target_format {
        "mp4" => original_size_mb * 0.85,  // H.264 usually ~85% of original
        "webm" => original_size_mb * 0.70, // VP9 usually ~70% of original
        "mkv" => original_size_mb * 0.90,  // MKV container, minimal change
        _ => original_size_mb,
    }
}

// Convert video to different format
#[tauri::command]
async fn convert_video(
    app_handle: tauri::AppHandle,
    video_path: String,
    target_format: String,
) -> Result<String, String> {
    let video_path_obj = Path::new(&video_path);
    let video_dir = video_path_obj
        .parent()
        .ok_or("Could not get video directory")?;
    let video_stem = video_path_obj
        .file_stem()
        .ok_or("Could not get video filename")?;

    // Output path
    let output_path = video_dir.join(format!(
        "{}_converted.{}",
        video_stem.to_string_lossy(),
        target_format
    ));
    let output_path_str = output_path.to_string_lossy().to_string();

    // Emit initial progress
    let _ = app_handle.emit(
        "conversion-progress",
        ConversionProgress {
            stage: "starting".to_string(),
            progress: 0.0,
            message: format!("Starting conversion to {}...", target_format.to_uppercase()),
        },
    );

    // Run conversion in blocking task
    let video_path_clone = video_path.clone();
    let output_path_clone = output_path_str.clone();
    let target_format_clone = target_format.clone();
    let app_handle_clone = app_handle.clone();

    tokio::task::spawn_blocking(move || {
        convert_video_with_ffmpeg(
            &video_path_clone,
            &output_path_clone,
            &target_format_clone,
            &app_handle_clone,
        )
    })
        .await
        .map_err(|e| format!("Conversion task failed: {}", e))??;

    // Emit completion
    let _ = app_handle.emit(
        "conversion-progress",
        ConversionProgress {
            stage: "complete".to_string(),
            progress: 100.0,
            message: "Conversion complete!".to_string(),
        },
    );

    Ok(output_path_str)
}

// Convert video using FFmpeg
fn convert_video_with_ffmpeg(
    input_path: &str,
    output_path: &str,
    target_format: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    let _ = app_handle.emit(
        "conversion-progress",
        ConversionProgress {
            stage: "converting".to_string(),
            progress: 50.0,
            message: format!("Converting to {}...", target_format.to_uppercase()),
        },
    );

    // Build FFmpeg command based on target format
    let mut cmd = create_hidden_command("ffmpeg");
    cmd.arg("-i").arg(input_path);

    match target_format {
        "mp4" => {
            cmd.args([
                "-c:v", "libx264", "-preset", "medium", "-crf", "23", "-c:a", "aac", "-b:a", "192k",
            ]);
        }
        "webm" => {
            cmd.args([
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "30",
                "-b:v",
                "0",
                "-c:a",
                "libopus",
            ]);
        }
        "mkv" => {
            cmd.args(["-c:v", "copy", "-c:a", "copy"]); // Just remux, no re-encoding
        }
        _ => return Err(format!("Unsupported format: {}", target_format)),
    }

    cmd.arg("-y").arg(output_path);

    let output = cmd.output().map_err(|e| {
        format!(
            "Failed to execute ffmpeg: {}. Make sure FFmpeg is installed.",
            e
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg conversion failed: {}", stderr));
    }

    Ok(())
}

// Get all watch progress data
#[tauri::command]
fn get_all_watch_progress() -> Result<std::collections::HashMap<String, WatchProgress>, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let progress_file = config_dir.join("watch_progress.json");

    if !progress_file.exists() {
        return Ok(std::collections::HashMap::new());
    }

    let content = fs::read_to_string(&progress_file)
        .map_err(|e| format!("Failed to read progress file: {}", e))?;

    let progress_map: std::collections::HashMap<String, WatchProgress> =
        serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new());

    Ok(progress_map)
}

#[tauri::command]
fn get_recent_videos() -> Result<Vec<VideoFile>, String> {
    let mut videos = Vec::new();

    // Check if ffprobe is available once at the start
    let ffprobe_available = create_hidden_command("ffprobe")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !ffprobe_available {
        eprintln!("Warning: ffprobe not found in PATH. Video durations will not be extracted.");
    }

    // Get common video directories
    let mut search_dirs = Vec::new();

    if let Some(home) = dirs::home_dir() {
        #[cfg(debug_assertions)]
        println!("Home directory: {:?}", home);
        search_dirs.push(home.join("Videos"));
        search_dirs.push(home.join("Downloads"));
        search_dirs.push(home.join("Desktop"));
        search_dirs.push(home.join("Documents"));
    }

    // Scan directories
    for dir in &search_dirs {
        #[cfg(debug_assertions)]
        println!("Checking directory: {:?} (exists: {})", dir, dir.exists());
        if dir.exists() {
            #[cfg(debug_assertions)]
            let mut dir_video_count = 0;
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() {
                            if let Some(ext) = entry.path().extension() {
                                if let Some(ext_str) = ext.to_str() {
                                    if MEDIA_EXTENSIONS.contains(&ext_str.to_lowercase().as_str()) {
                                        if let Ok(modified) = metadata.modified() {
                                            if let Ok(duration) =
                                                modified.duration_since(SystemTime::UNIX_EPOCH)
                                            {
                                                let video_path =
                                                    entry.path().to_string_lossy().to_string();
                                                // Only try to get duration if ffprobe is available
                                                let video_duration = if ffprobe_available {
                                                    get_video_duration(&video_path)
                                                } else {
                                                    None
                                                };

                                                videos.push(VideoFile {
                                                    path: video_path,
                                                    name: entry
                                                        .file_name()
                                                        .to_string_lossy()
                                                        .to_string(),
                                                    size: metadata.len(),
                                                    modified: duration.as_secs(),
                                                    duration: video_duration,
                                                });
                                                #[cfg(debug_assertions)]
                                                {
                                                    dir_video_count += 1;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            #[cfg(debug_assertions)]
            println!("Found {} videos in {:?}", dir_video_count, dir);
        }
    }

    // Sort by modified time (most recent first)
    videos.sort_by(|a, b| b.modified.cmp(&a.modified));

    // Return top 20
    videos.truncate(20);

    Ok(videos)
}

// Function to process video files and emit events
fn process_video_files(app_handle: &tauri::AppHandle, video_files: Vec<String>) {
    if !video_files.is_empty() {
        // Store in global queue
        {
            let mut pending = PENDING_FILES.lock().unwrap();
            for video_file in &video_files {
                pending.push_back(video_file.clone());
            }
        }

        // Spawn background thread for persistent file loading attempts
        let app_handle_clone = app_handle.clone();
        thread::spawn(move || {
            for attempt in 1..=MAX_FILE_LOADING_ATTEMPTS {
                // Check if file has been processed
                {
                    let processed = FILE_PROCESSED.lock().unwrap();
                    if *processed {
                        #[cfg(debug_assertions)]
                        println!("File already processed, stopping attempts");
                        break;
                    }
                }

                // Try to emit event for first video file immediately on first attempt
                if let Some(video_file) = video_files.first() {
                    #[cfg(debug_assertions)]
                    match app_handle_clone.emit("open-file", video_file) {
                        Ok(_) => {
                            println!(
                                "Attempt {}: Successfully emitted open-file for {}",
                                attempt, video_file
                            );
                        }
                        Err(e) => {
                            println!("Attempt {}: Failed to emit event: {:?}", attempt, e);
                        }
                    }
                    #[cfg(not(debug_assertions))]
                    let _ = app_handle_clone.emit("open-file", video_file);
                }

                // Wait before next attempt (no delay before first attempt)
                if attempt < MAX_FILE_LOADING_ATTEMPTS {
                    let delay = if attempt == 1 {
                        // First retry: wait for frontend to be fully ready
                        Duration::from_millis(FRONTEND_READY_WAIT_MS)
                    } else if attempt <= INITIAL_ATTEMPT_COUNT {
                        Duration::from_millis(INITIAL_ATTEMPT_DELAY_MS)
                    } else if attempt <= MIDDLE_ATTEMPT_COUNT {
                        Duration::from_millis(MIDDLE_ATTEMPT_DELAY_MS)
                    } else {
                        Duration::from_millis(FINAL_ATTEMPT_DELAY_MS)
                    };

                    thread::sleep(delay);
                }
            }

            #[cfg(debug_assertions)]
            println!("File loading attempts completed");
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init());

    // Add updater plugin only on Windows
    #[cfg(target_os = "windows")]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|app| {
            // Handle command line arguments for file associations
            let args: Vec<String> = std::env::args().collect();

            #[cfg(debug_assertions)]
            {
                println!("=== GLUCOSE STARTUP ===");
                println!("Command line arguments: {:?}", args);
            }

            // Check if we're being launched via file association
            if args.len() > 1 {
                #[cfg(debug_assertions)]
                println!("*** LAUNCHED WITH ARGUMENTS - POTENTIAL FILE ASSOCIATION ***");

                let mut video_files: Vec<String> = Vec::new();

                for arg in &args[1..] {
                    // Sanitize the path
                    let clean_arg = sanitize_path(arg);
                    #[cfg(debug_assertions)]
                    println!("Processing argument: {} -> {}", arg, clean_arg);

                    let lower = clean_arg.to_lowercase();
                    for ext in MEDIA_EXTENSIONS {
                        if lower.ends_with(&format!(".{}", ext)) {
                            video_files.push(clean_arg.clone());
                            #[cfg(debug_assertions)]
                            println!("Found video file: {}", clean_arg);
                            break;
                        }
                    }
                }

                if !video_files.is_empty() {
                    #[cfg(debug_assertions)]
                    println!("Queued {} video files", video_files.len());
                    process_video_files(&app.handle(), video_files);
                }
            } else {
                #[cfg(debug_assertions)]
                println!("*** LAUNCHED WITHOUT ARGUMENTS - NORMAL APP LAUNCH ***");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_file_dialog,
            open_subtitle_dialog,
            convert_file_path,
            get_recent_videos,
            get_pending_file,
            mark_file_processed,
            frontend_ready,
            exit_app,
            find_subtitle_for_video,
            get_embedded_subtitle_tracks,
            extract_embedded_subtitle,
            save_watch_progress,
            get_watch_progress,
            get_all_watch_progress,
            get_video_info,
            convert_video,
            enter_pip_mode,
            exit_pip_mode
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // Log events for debugging
            if cfg!(debug_assertions) {
                println!("Received event: {:?}", event);
            }

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            match event {
                // Handle macOS file association events
                RunEvent::Opened { urls } => {
                    #[cfg(debug_assertions)]
                    {
                        println!("*** FILE ASSOCIATION EVENT RECEIVED ***");
                        println!("Received opened event with URLs: {:?}", urls);
                    }

                    let mut video_files: Vec<String> = Vec::new();

                    for url in urls {
                        let url_str = url.to_string();
                        #[cfg(debug_assertions)]
                        println!("Processing URL: {}", url_str);

                        if url_str.starts_with("file://") {
                            let path = url_str.replace("file://", "");
                            let decoded_path = urlencoding::decode(&path).unwrap_or_default();

                            #[cfg(debug_assertions)]
                            println!("Decoded path: {}", decoded_path);

                            let lower = decoded_path.to_lowercase();
                            for ext in MEDIA_EXTENSIONS {
                                if lower.ends_with(&format!(".{}", ext)) {
                                    video_files.push(decoded_path.to_string());
                                    #[cfg(debug_assertions)]
                                    println!(
                                        "Found video file from opened event: {}",
                                        decoded_path
                                    );
                                    break;
                                }
                            }
                        }
                    }

                    if !video_files.is_empty() {
                        #[cfg(debug_assertions)]
                        println!(
                            "Processing {} video files from file association event",
                            video_files.len()
                        );
                        process_video_files(&_app_handle, video_files);
                    }
                }
                _ => {}
            }
        });
}
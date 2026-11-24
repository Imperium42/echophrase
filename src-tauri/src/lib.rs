#![allow(clippy::too_many_arguments)] // Complex functions sometimes need many params
#![allow(clippy::collapsible_if)] // Explicit if nesting can be clearer
#![allow(clippy::derivable_impls)] // Explicit Default impls are fine
#![allow(clippy::manual_flatten)] // Sometimes explicit loops are clearer
#![allow(clippy::useless_format)] // Allows format! for future expansion
#![allow(clippy::needless_borrow)] // Sometimes explicit borrows are clearer
#![allow(clippy::needless_return)] // Sometimes explicit returns are clearer
#![allow(clippy::ptr_arg)] // &PathBuf vs &Path is fine in many contexts
#![allow(clippy::wrong_self_convention)] // to_* methods may take &self
#![allow(clippy::iter_cloned_collect)] // iter().copied() vs to_vec() is fine
#![allow(clippy::needless_borrows_for_generic_args)] // Explicit borrows for clarity

// SPDX-FileCopyrightText: 2025-2026 Imperium42 LLC
//
// SPDX-License-Identifier: MPL-2.0

mod audio;
mod commands;
mod error;
mod gpu_probe;
mod ipc;
#[cfg(target_os = "macos")]
mod macos_permissions;
mod model_detection;
mod model_download;
mod models;
mod platform;
mod settings;
mod setup;
mod shutdown;
mod transcription;
mod transform;
mod tray_status;
mod types;
mod vad;
// Cross-platform module (command is a no-op off macOS) so the frontend can
// invoke set_dictation_key_remap unconditionally.
mod macos_dictation;
#[cfg(test)]
mod tests;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

/// Whether the ONNX Runtime dylib can be loaded on this system.
/// Set during validate_environment() via dlopen probe.
/// When false, ONNX-based transcription and VAD should return errors
/// instead of panicking.
pub static ONNX_RUNTIME_AVAILABLE: AtomicBool = AtomicBool::new(true);

/// Check if ONNX Runtime is available on this system.
pub fn is_onnx_runtime_available() -> bool {
    ONNX_RUNTIME_AVAILABLE.load(Ordering::Relaxed)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 0a) Single-instance guard (file-lock based, works on WSL where D-Bus doesn't).
    //     Must run before any expensive init so a duplicate launch exits fast.
    if !setup::instance_lock::acquire_instance_lock() {
        std::process::exit(1);
    }

    // 0a.5) Suppress Windows Error Reporting popups for child processes. When a
    //       child (e.g. a third-party tool we shell out to) fails DllMain init
    //       with STATUS_DLL_INIT_FAILED (0xC0000142) - common with stale NVIDIA
    //       driver pieces - Windows pops a modal "Application Error" dialog we
    //       cannot suppress per-spawn from std::process::Command. Setting the
    //       process error mode here propagates to children unless they're
    //       launched with CREATE_DEFAULT_ERROR_MODE (we never set that flag).
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::System::Diagnostics::Debug::{
            SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SetErrorMode,
        };
        // SAFETY: SetErrorMode is process-global and thread-safe. Returns the
        // previous mode which we discard - we don't need to restore it.
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }

    // 0b) SIGINT/SIGTERM handler - terminal Ctrl+C in `bun run tauri:dev` currently
    //     kills the shell/node process but can leave the GUI child orphaned. This
    //     funnels signals through our force_exit so shortcuts get unregistered.
    if let Err(e) = ctrlc::set_handler(|| {
        log::info!("SIGINT/SIGTERM received - routing through force_exit");
        shutdown::force_exit(130); // 128+SIGINT
    }) {
        log::warn!(
            "Failed to install SIGINT handler ({}) - Ctrl+C may orphan the GUI process",
            e
        );
    }

    // 1) Pick which ORT runtime to load (dev override or bundled).
    setup::ort_runtime::select_ort_runtime();
    // 2) Preload ORT provider DLLs before any ORT API is touched.
    setup::ort_runtime::preload_ort_providers();

    // Validate environment configuration (debug mode only - fails fast on errors)
    #[cfg(debug_assertions)]
    {
        if let Err(e) = setup::env_validation::validate_environment() {
            eprintln!("Startup validation failed: {}", e);
            eprintln!("Run: bun run env:check-deps");
            shutdown::force_exit(1);
        }
    }

    // In production, validate but don't fail - just log warnings
    #[cfg(not(debug_assertions))]
    {
        let _ = setup::env_validation::validate_environment();
    }

    // Create the audio recorder instance
    let recorder = Arc::new(tokio::sync::Mutex::new(audio::AudioRecorder::new()));

    // Create VAD test state for audio input settings
    let vad_state = Arc::new(vad::VadTestState::new());

    // Warm the settings cache so commands get a cache hit on first call.
    let settings_cache = settings::cache::SettingsCache::init();

    tauri::Builder::default()
    .manage(commands::recording::RecorderState(recorder))
    .manage(commands::vad::VadState(vad_state))
    .manage(settings_cache)
    .setup(|app| {
      // Stash AppHandle for force_exit's shortcut-cleanup path (see shutdown.rs).
      shutdown::install_app_handle(app.handle().clone());

      {
        // Log to app data dir (~/.local/share/com.imperium42.echophrase/logs/)
        let log_dir = app.path().app_log_dir().unwrap_or_else(|_| "../logs".into());

        // Rotate previous session logs (keep one old copy for crash diagnosis)
        let log_file = log_dir.join("Echophrase.log");
        let old_log_file = log_dir.join("Echophrase-old.log");
        if log_file.exists() {
          let _ = std::fs::rename(&log_file, &old_log_file);
        }

        let mut targets = vec![
          tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Folder {
            path: log_dir,
            file_name: None,
          }),
        ];

        // Stdout and webview logging only in dev builds
        if cfg!(debug_assertions) {
          targets.push(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout));
          targets.push(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Webview));
        }

        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            // Inference/audio internals log at debug; keep them out of release
            // log files but visible in dev.
            .level(if cfg!(debug_assertions) {
              log::LevelFilter::Debug
            } else {
              log::LevelFilter::Info
            })
            .targets(targets)
            .build(),
        )?;

        // Re-emit the runtime-selection diagnostic captured before the log
        // plugin existed (stderr is NUL on Windows GUI builds, so this is the
        // only way the chosen ORT DLL ends up in Echophrase.log).
        #[cfg(target_os = "windows")]
        if let Some(msg) = setup::ort_runtime::RUNTIME_SELECTION_DIAGNOSTIC.get() {
          log::info!("{}", msg);
        }
      }

      // Setup system tray with menu
      setup::tray::setup_tray(app)?;

      // Free the model's memory after 10 idle minutes (reloaded on next recording).
      transcription::idle::spawn_watcher(app.handle().clone());

      // Configure main window (stays hidden until frontend calls show_main_window)
      if let Some(window) = app.get_webview_window("main") {
        // Enforce minimum window size (WSL/Linux WMs sometimes ignore config)
        use tauri::{LogicalSize, Size};
        let min_size: Size = LogicalSize::new(475.0, 400.0).into();
        let _ = window.set_min_size(Some(min_size));

        // In dev mode, continuously monitor dev server and auto-reconnect when available
        #[cfg(debug_assertions)]
        {
          let window_clone = window.clone();
          std::thread::spawn(move || {
            // Same DEV_PORT the launcher (scripts/env/load.mjs) and vite.config.js use.
            let dev_port: String = std::env::var("DEV_PORT").unwrap_or_else(|_| "8420".to_string());
            let dev_url: String = format!("http://localhost:{}", dev_port);
            let mut was_connected = false;
            let mut reconnect_logged = false;

            println!("[Dev] Monitor thread started, polling {}", dev_url);
            eprintln!("[Dev] Monitor thread started, polling {}", dev_url);

            loop {
              let result = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_millis(500))
                .build()
                .and_then(|client| client.get(&dev_url).send());

              let is_connected = match &result {
                Ok(r) => r.status().is_success(),
                Err(_) => false,
              };

              if is_connected && !was_connected {
                // Server just became available - navigate to it
                println!("[Dev] Server {} - navigating to {}",
                  if reconnect_logged { "reconnected" } else { "ready" }, dev_url);

                match window_clone.navigate(dev_url.parse().unwrap()) {
                  Ok(_) => println!("[Dev] Navigate succeeded"),
                  Err(e) => {
                    println!("[Dev] Navigate failed: {}, trying eval reload", e);
                    let _ = window_clone.eval("window.location.reload()");
                  }
                }
                reconnect_logged = true;
              } else if !is_connected && was_connected {
                println!("[Dev] Server disconnected - waiting for reconnect...");
              }

              was_connected = is_connected;
              std::thread::sleep(std::time::Duration::from_millis(1000));
            }
          });
        }

        // Intercept window close ("X" button): optionally hide to tray instead of exiting.
        // When minimize_to_tray_on_close is enabled (default), the window hides and on the
        // first hide ever we fire a native OS notification so the user knows where the app went.
        // Tray menu "Quit" remains the only path to actually terminate the app
        // (funneled through shutdown::force_exit - see that module for why
        // std::process::exit / app.exit / RunEvent::Exit all deadlock).
        //
        // Linux exception: WSL (and many tray-less Linux setups) hide the tray
        // icon in a "mysterious place," so closing-to-tray strands the process.
        // Quit cleanly on Linux instead.
        let window_for_close = window.clone();
        let app_handle_for_close = app.handle().clone();
        window.on_window_event(move |event| {
          if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            #[cfg(target_os = "linux")]
            {
              let _ = api;
              let _ = &window_for_close;
              let _ = &app_handle_for_close;
              log::info!("Window close requested (Linux) - quitting app (no tray on WSL)");
              crate::shutdown::force_exit(0);
            }
            #[cfg(not(target_os = "linux"))]
            {
              // Read the current minimize_to_tray_on_close preference.
              // Default to true if settings cannot be read.
              let minimize_to_tray = settings::AppSettings::load()
                .map(|s| s.minimize_to_tray_on_close)
                .unwrap_or(true);

              if minimize_to_tray {
                log::info!("Window close requested - hiding to tray");
                api.prevent_close();
                let _ = window_for_close.hide();

                // Show a one-time OS notification so the user knows the app is
                // still running in the tray. After the first time, stay quiet.
                crate::setup::tray::maybe_send_tray_notice(&app_handle_for_close);
              } else {
                log::info!("Window close requested - quitting (minimize_to_tray_on_close disabled)");
                // Let the default close action proceed (quits the app).
                let _ = api;
                crate::shutdown::force_exit(0);
              }
            }
          }
        });

        // Register global shortcuts (F5 reload in dev mode)
        setup::shortcuts::register_shortcuts(app);
        // macOS dictation-key (F13) takeover is driven by the user's binding via
        // the set_dictation_key_remap command - the mic key flows through the
        // normal user-hotkey global-shortcut path, not a hardcoded handler.
      }

      // Start the localhost-only IPC server (CLI `echophrase transcribe` /
      // MCP `transcribe` tool). Routes through transcribe_source so the
      // user's selected model (Qwen3/ONNX/Candle) is honored, same
      // as in-app dictation. Preview-less (no AppHandle).
      {
        use std::path::PathBuf;
        let transcribe: ipc::server::TranscribeFn = std::sync::Arc::new(|path: PathBuf| {
          Box::pin(async move {
            let source = echo_audio::AudioSource::from_path(path);
            transcription::transcribe_source(&source, None).await
          })
        });
        ipc::spawn(transcribe, env!("CARGO_PKG_VERSION"));
      }

      // Create splash window (instant - inline data: URL HTML, no Vite/SvelteKit,
      // no filesystem dependency). Main window stays hidden until frontend
      // calls show_main_window.
      //
      // Previously this wrote the HTML to a fixed shared temp path
      // (echophrase_splash.html) and navigated to it via a file:// URL. That
      // caused occasional WebView2 "File not found ERR_FILE_NOT_FOUND" errors
      // on Windows: AV scanning could lock/quarantine the just-written file
      // before navigation completed, or an updater relaunch/double-launch
      // could delete the same fixed-name file out from under the first
      // process's in-flight navigation. A data: URL has no file to lock,
      // quarantine, or race, so the whole failure class is gone.
      match setup::splash::create_splash_window(app) {
        Ok(_) => log::info!("Splash window created"),
        Err(e) => log::warn!("Splash window unavailable: {}", e),
      }

      // Splash watchdog: if the frontend never calls show_main_window
      // (e.g. due to a SyntaxError aborting the module graph), force-show
      // the main window so the user isn't stuck on an infinite spinner.
      // Dev: 10s + open devtools. Prod: 20s, silent recovery.
      {
        use tauri::Manager;
        let app_handle = app.handle().clone();
        #[cfg(debug_assertions)]
        let timeout_secs: u64 = 10;
        #[cfg(not(debug_assertions))]
        let timeout_secs: u64 = 20;

        std::thread::spawn(move || {
          std::thread::sleep(std::time::Duration::from_secs(timeout_secs));
          if let Some(main) = app_handle.get_webview_window("main") {
            let visible = main.is_visible().unwrap_or(false);
            if !visible {
              log::warn!(
                "Splash watchdog: main window still hidden after {}s - force-showing (frontend likely failed to boot)",
                timeout_secs
              );
              let _ = main.unminimize();
              let _ = main.show();
              let _ = main.set_focus();
              if let Some(splash) = app_handle.get_webview_window("splash") {
                let _ = splash.close();
              }
              #[cfg(debug_assertions)]
              {
                main.open_devtools();
              }
            }
          }
        });
      }

      #[cfg(windows)]
      commands::output::focus::start_tracker();

      log::info!("Tauri app setup complete with native microphone plugin and system tray");
      Ok(())
    })
    .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
      // Race window: a second-instance launch can fire before `setup()`
      // has built the main window. unwrap() here would panic the running
      // process (the first instance!). Skip silently if not yet ready.
      if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
      } else {
        log::warn!("single-instance: main window not yet created, skipping show/focus");
      }
    }))
    // REMOVED: tauri_plugin_shell::init() - spawns cmd.exe on Windows
    .plugin(tauri_plugin_clipboard_manager::init())
    .plugin(tauri_plugin_global_shortcut::Builder::new()
      .with_handler(|app, shortcut, event| {
        // F5 reload handler (dev only, registered in setup)
        if shortcut.key == tauri_plugin_global_shortcut::Code::F5
          && event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed
        {
          log::info!("F5 pressed - reloading webview");
          if let Some(window) = app.get_webview_window("main") {
            let _ = window.eval("window.location.reload()");
          }
        }
      })
      .build())
    .plugin(tauri_plugin_notification::init())
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_process::init())
    .plugin(tauri_plugin_updater::Builder::new().build())
    // REMOVED: tauri_plugin_opener::init() - uses cmd.exe internally on Windows
    .invoke_handler(tauri::generate_handler![
      macos_dictation::set_dictation_key_remap,
      setup::window::show_main_window,
      commands::window::quit_app,
      commands::transcription::transcribe_audio,
      commands::transcription::check_whisper_model_exists,
      commands::transcription::check_selected_model_exists,
      commands::transcription::check_models_exist,
      commands::transcription::download_whisper_model,
      commands::transcription::cancel_model_download,
      commands::transcription::switch_model_download,
      commands::transcription::get_debug_logs,
      commands::transcription::check_microphone_available, // forwarder to audio_devices - see TODO in commands/mod.rs
      commands::transcription::delete_model,
      commands::transcription::check_partial_download,
      commands::transcription::delete_partial_download,
      commands::transcription::preload_whisper_model,
      commands::transcription::clear_whisper_model_cache,
      commands::transcription::unload_model,
      commands::transcription::get_audio_input_devices, // forwarder to audio_devices - see TODO in commands/mod.rs
      commands::audio_devices::request_microphone_permission,
      commands::audio_devices::get_microphone_permission_status,
      commands::audio_devices::open_microphone_system_settings,
      commands::recording::start_audio_recording,
      commands::recording::stop_audio_recording_and_transcribe,
      commands::recording::start_audio_recording_with_vad,
      commands::recording::transcribe_open_mic_chunk,
      commands::recording::reset_recording_state,
      commands::recording::is_wsl_environment,
      commands::recording::get_system_info,
      commands::recording::is_wsl,
      commands::recording::open_url,
      commands::recording::open_url_wsl,
      commands::recording::open_url_windows,
      commands::recording::open_url_linux,
      commands::recording::open_url_mac,
      commands::recording::copy_to_windows_clipboard,
      commands::recording::check_ort_available,
      commands::managed_install::get_managed_install,
      settings::get_settings,
      settings::save_settings,
      settings::update_settings_patch,
      settings::set_mcp_server_enabled,
      settings::get_language_setting,
      settings::save_language_setting,
      settings::get_system_locale,
      // BYO custom model commands
      commands::custom_model::import_custom_model_from_file,
      commands::custom_model::import_custom_model_from_url,
      commands::custom_model::list_hf_onnx_files,
      commands::custom_model::check_custom_model_partial_download,
      commands::custom_model::delete_custom_model_partial_download,
      commands::custom_model::get_custom_model_dir_path,
      commands::custom_model::remove_custom_model,
      commands::custom_model::get_custom_model_info,
      commands::custom_model::list_custom_models,
      commands::custom_model::select_custom_model,
      // Window management commands (devtools in staging + dev builds)
      #[cfg(any(feature = "devtools", debug_assertions))]
      commands::window::toggle_devtools,
      #[cfg(any(feature = "devtools", debug_assertions))]
      commands::window::debug_show_splash,
      // Output handling commands
      commands::output::simulate_paste,
      commands::output::get_paste_permission_granted,
      commands::output::request_paste_permission,
      commands::output::open_accessibility_system_settings,
      commands::output::save_transcript_dialog,
      commands::output::save_transcript_to_directory,
      commands::output::pick_directory,
      commands::output::open_directory,
      commands::transcription::get_models_dir,
      // VAD test commands for audio input settings
      commands::vad::start_vad_test,
      commands::vad::stop_vad_test,
      commands::vad::set_vad_threshold,
      commands::vad::is_vad_test_running,
      // Model auto-detection commands
      commands::model_detection::maybe_auto_detect_model,
      commands::model_detection::force_auto_detect_model,
      commands::model_detection::get_model_recommendation,
      commands::model_detection::reset_model_detection,
      // Execution provider / GPU selection commands
      commands::execution_provider::get_execution_provider,
      commands::execution_provider::set_preferred_execution_provider,
      commands::execution_provider::list_ort_devices,
      // CT2RS commands disabled - using candle instead
      // commands::transcription_ct2::transcribe_audio_ct2,
      // commands::transcription_ct2::download_whisper_model_ct2,
      // commands::transcription_ct2::check_ct2_model_exists
      // Transform (grammar correction) commands
      commands::transform::transform_text,
      commands::transform::preload_transform_model,
      commands::transform::is_transform_model_loaded,
      commands::transform::unload_transform_model,
    ])
    .build(tauri::generate_context!())
    .expect("error while building tauri application")
    .run({
      // Set once tauri itself asks to exit/restart (e.g. relaunch() after an
      // update); those must reach tauri's own Exit handling untouched.
      let mut exit_requested_seen = false;
      move |_app, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
          exit_requested_seen = true;
        }
        // An event loop torn down from outside the app (Windows logoff /
        // shutdown / Restart Manager) must end here, or tao panics on the
        // next event. See shutdown::exit_code_for_run_event.
        if let Some(code) = shutdown::exit_code_for_run_event(&event, exit_requested_seen) {
          log::info!("Event loop destroyed outside our quit paths - routing through force_exit");
          shutdown::force_exit(code);
        }
      }
    });
}

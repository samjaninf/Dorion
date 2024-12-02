#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod linux;

use tauri::Manager;
use tauri_plugin_window_state::{StateFlags, WindowExt};

use crate::{
  config::get_config,
  injection::plugin::load_plugins,
  log,
  util::{
    color::start_os_accent_subscriber,
    window_helpers::{set_user_agent, ultrashow, window_zoom_level},
  },
};

#[cfg(feature = "blur")]
use crate::window::blur::apply_effect;

#[cfg(feature = "rpc")]
#[cfg(not(target_os = "macos"))]
use super::rpc::start_rpc_server;
use super::{extension::load_extensions, tray::create_tray};

pub fn configure(window: &tauri::WebviewWindow) {
  let safemode = std::env::args().any(|arg| arg == "--safemode");
  let startup = std::env::args().any(|arg| arg == "--startup");
  let config = get_config();
  let handle = window.app_handle();

  // Set the user agent to one that enables all normal Discord features
  set_user_agent(window);

  // Multi-instance check
  if !config.multi_instance.unwrap_or(false) {
    log!("Multi-instance disabled, registering single instance plugin...");

    handle
      .plugin(tauri_plugin_single_instance::init(
        move |app, _argv, _cwd| {
          if let Some(win) = app.get_webview_window("main") {
            ultrashow(win);
          }
        },
      ))
      .unwrap_or_else(|_| log!("Failed to register single instance plugin"));
  }

  // If safemode is enabled, stop here
  if safemode {
    window.show().unwrap_or_default();
    return;
  }

  // Restore state now in case we do window modification (ie maximize) later
  window.restore_state(StateFlags::all()).unwrap_or_default();

  load_extensions(window);
  load_plugins(window.clone(), Some(true));

  // begin the RPC server if needed
  #[cfg(feature = "rpc")]
  #[cfg(not(target_os = "macos"))]
  if config.rpc_server.unwrap_or(false) {
    let win = window.clone();
    std::thread::spawn(|| {
      start_rpc_server(win);
    });
  }

  // If the subscription is dropped, Mundy's internal thread will exit and no events will ever be recieved
  Box::leak(Box::new(start_os_accent_subscriber(window)));

  #[cfg(feature = "hotkeys")]
  #[cfg(not(target_os = "macos"))]
  if config.keybinds_enabled.unwrap_or(false) {
    log!("Starting global keybind watcher...");
    super::hotkeys::start_keybind_watcher(window);
  }

  // If we are opening on startup (which we know from the --startup arg), check to keep the window minimized
  if !startup || !config.startup_minimized.unwrap_or(false) {
    // Now that we are ready, and shouldn't start minimized, show the window
    window.show().unwrap_or_default();
  } else {
    window.hide().unwrap_or_default();
  }

  if config.start_maximized.unwrap_or(false) {
    window.maximize().unwrap_or_default();
  }

  #[cfg(feature = "blur")]
  apply_effect(
    window.clone(),
    config.blur.unwrap_or("none".to_string()).as_str(),
  );

  create_tray(handle).unwrap_or_else(|e| log!("Error creating tray icon: {:?}", e));

  #[cfg(target_os = "windows")]
  windows::configure(window);

  #[cfg(target_os = "macos")]
  macos::configure(window);

  #[cfg(target_os = "linux")]
  linux::configure(window);

  window_zoom_level(window.clone(), None);
}
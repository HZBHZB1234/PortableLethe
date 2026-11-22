pub mod commands;
pub mod core;
pub mod error;
pub mod model;
pub mod state;

use tauri::Manager;
use crate::core::Hub;
use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let hub = Hub::new().map_err(|e| e.to_string())?;
            let state = AppState::new(hub).map_err(|e| e.to_string())?;
            *state.handle.lock().map_err(|_| "log handle poisoned")? = Some(app.handle().clone());
            state.log("info", "hub", "Portable Lethe Hub starting");
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::hub_status,
            commands::hub_reveal_dir,
            commands::hub_config_schema,
            commands::hub_config_get,
            commands::hub_config_set,
            commands::hub_config_update,
            commands::instance_list,
            commands::instance_create,
            commands::instance_start,
            commands::instance_stop,
            commands::instance_restart,
            commands::instance_delete,
            commands::instance_open_admin,
            commands::version_list,
            commands::version_install,
            commands::version_build_from_source,
            commands::version_switch,
            commands::version_rollback,
            commands::resource_status,
            commands::resource_sync,
            commands::clientpkg_export,
            commands::log_tail,
            commands::request_restart,
            commands::catalog_find,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Portable Lethe Hub");
}

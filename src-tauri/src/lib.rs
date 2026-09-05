#![cfg_attr(not(feature = "desktop"), allow(dead_code))]

mod commands;
mod models;
mod systemd;

#[cfg(feature = "desktop")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::get_services,
            commands::get_service_status,
            commands::start_service,
            commands::stop_service,
            commands::restart_service,
            commands::enable_service,
            commands::disable_service,
            commands::start_all_services,
            commands::stop_all_services,
            commands::restart_all_services,
            commands::enable_all_services,
            commands::disable_all_services,
        ])
        .run(tauri::generate_context!())
        .expect("error while running BatRing");
}

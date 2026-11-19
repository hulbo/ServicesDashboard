mod commands;
mod importers;
mod job;
mod launch;
mod logs;
mod model;
mod process;
mod runtime;
mod store;
mod vars;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Manager, RunEvent};

use commands::{AppCore, TauriSink};
use logs::LogHub;
use process::{EventSink, ProcessManager};
use store::Store;

/// 로그를 UI로 모아 보내는 주기
const LOG_FLUSH_INTERVAL: Duration = Duration::from_millis(50);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let store = Store::open(Store::default_base_dir())?;
            let logs = Arc::new(LogHub::new(logs::DEFAULT_CAPACITY));
            let sink: Arc<dyn EventSink> = Arc::new(TauriSink(app.handle().clone()));
            let manager = ProcessManager::new(logs.clone(), sink.clone());
            let workspace = store.load();
            let core = Arc::new(AppCore {
                store,
                workspace: Mutex::new(workspace),
                manager,
                logs: logs.clone(),
                runtimes: Mutex::new(None),
                detect_env: runtime::DetectEnv::from_system(),
            });
            app.manage(core.clone());

            // 런타임 감지는 수백 ms~수 초 걸릴 수 있어 미리 해 둔다.
            std::thread::Builder::new().name("runtime-detect".into()).spawn(move || {
                core.runtimes(false);
            })?;

            std::thread::Builder::new().name("log-flush".into()).spawn(move || {
                loop {
                    std::thread::sleep(LOG_FLUSH_INTERVAL);
                    let batches = logs.take_pending();
                    if !batches.is_empty() {
                        sink.logs(batches);
                    }
                }
            })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::workspace_get,
            commands::workspace_reload,
            commands::workspace_save,
            commands::service_statuses,
            commands::service_start,
            commands::service_stop,
            commands::service_restart,
            commands::group_start,
            commands::group_stop,
            commands::stop_all,
            commands::logs_get,
            commands::logs_clear,
            commands::launch_preview,
            commands::runtimes_detect,
            commands::project_scan,
            commands::open_path,
            commands::open_url,
        ])
        .build(tauri::generate_context!())
        .expect("Services Dashboard를 시작할 수 없습니다");

    app.run(|handle, event| {
        // 앱이 끝나면 모든 서비스 프로세스 트리를 종료한다.
        // (Job Object의 KILL_ON_JOB_CLOSE가 있어 크래시 때도 정리된다)
        if let RunEvent::Exit = event {
            if let Some(core) = handle.try_state::<Arc<AppCore>>() {
                core.manager.kill_all_now();
            }
        }
    });
}

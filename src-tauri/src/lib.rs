mod commands;
mod git;
mod importers;
mod javamain;
mod job;
mod launch;
mod logfile;
mod logs;
mod model;
mod ports;
mod process;
mod runtime;
mod shell;
mod store;
mod vars;
mod watch;

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
    let mut builder = tauri::Builder::default();
    // 두 번째로 실행하면 새 창 대신 이미 떠 있는 창을 앞으로 가져온다 (첫 번째로 등록해야 한다).
    // SERVICES_DASHBOARD_HOME으로 설정 폴더를 따로 쓰는 실행(테스트·휴대용)은 설정이 겹치지 않으므로 따로 띄운다.
    if !store::Store::uses_custom_base_dir() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| shell::show_main(app)));
    }
    let app = builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .on_window_event(shell::on_window_event)
        .setup(|app| {
            let store = Store::open(Store::default_base_dir())?;
            let settings = store.settings.clone();
            let files = Arc::new(logfile::LogFiles::new(
                store.logs_dir(),
                settings.log_files.enabled,
                settings.log_files.retention_days,
            ));
            files.cleanup();
            let logs = Arc::new(LogHub::new(logs::DEFAULT_CAPACITY).with_files(files.clone()));
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
                settings: Mutex::new(settings),
                files: files.clone(),
                restarts: Mutex::new(Default::default()),
                on_error: {
                    let handle = app.handle().clone();
                    Box::new(move |e| {
                        use tauri::Emitter;
                        let _ = handle.emit(commands::EVENT_START_ERROR, e);
                    })
                },
            });
            app.manage(core.clone());
            shell::setup_tray(app.handle())?;

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
                    files.flush_if_due();
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
            commands::preset_start,
            commands::preset_stop,
            commands::project_export,
            commands::stop_all,
            commands::logs_get,
            commands::logs_clear,
            commands::launch_preview,
            commands::runtimes_detect,
            commands::project_scan,
            commands::ports_check,
            commands::process_kill,
            commands::settings_get,
            commands::settings_save,
            commands::logs_export,
            commands::logs_dir,
            commands::open_path,
            commands::open_url,
            commands::git_repos,
            commands::git_branches,
            commands::git_fetch,
            commands::git_switch,
        ])
        .build(tauri::generate_context!())
        .expect("Services Dashboard를 시작할 수 없습니다");

    app.run(|handle, event| {
        // 앱이 끝나면 모든 서비스 프로세스 트리를 종료한다.
        // (Job Object의 KILL_ON_JOB_CLOSE가 있어 크래시 때도 정리된다)
        if let RunEvent::Exit = event {
            if let Some(core) = handle.try_state::<Arc<AppCore>>() {
                core.manager.kill_all_now();
                core.files.flush();
            }
        }
    });
}

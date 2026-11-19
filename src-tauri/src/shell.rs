//! 앱 셸: 트레이 아이콘·메뉴, 창 닫기 → 트레이로, 알림.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, Window, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use crate::commands::AppCore;
use crate::process::{RunState, ServiceStatus};

pub const TRAY_ID: &str = "main";

fn core<R: Runtime>(app: &AppHandle<R>) -> Option<Arc<AppCore>> {
    app.try_state::<Arc<AppCore>>().map(|s| s.inner().clone())
}

pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn notify<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn build_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(app, "show", "Services Dashboard 열기", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    if let Some(ws) = core(app).and_then(|c| c.workspace.lock().unwrap().clone().ok()) {
        for g in &ws.groups {
            let start = MenuItem::with_id(app, format!("gstart:{}", g.id), "전체 시작", true, None::<&str>)?;
            let stop = MenuItem::with_id(app, format!("gstop:{}", g.id), "전체 중지", true, None::<&str>)?;
            menu.append(&Submenu::with_items(app, &g.name, true, &[&start, &stop])?)?;
        }
        if !ws.groups.is_empty() {
            menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
        if !ws.presets.is_empty() {
            let items: Vec<MenuItem<R>> = ws
                .presets
                .iter()
                .map(|p| MenuItem::with_id(app, format!("pstart:{}", p.id), &p.name, true, None::<&str>))
                .collect::<tauri::Result<_>>()?;
            let refs: Vec<&dyn tauri::menu::IsMenuItem<R>> = items.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<R>).collect();
            menu.append(&Submenu::with_items(app, "프리셋 시작", true, &refs)?)?;
            menu.append(&PredefinedMenuItem::separator(app)?)?;
        }
    }
    menu.append(&MenuItem::with_id(app, "stop_all", "실행 중인 서비스 모두 중지", true, None::<&str>)?)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(app, "quit", "종료 (서비스도 모두 중지)", true, None::<&str>)?)?;
    Ok(menu)
}

pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Services Dashboard")
        .menu(&build_menu(app)?)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id.as_ref()))
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// 그룹이 바뀌면 트레이 메뉴를 다시 만든다.
pub fn refresh_tray<R: Runtime>(app: &AppHandle<R>) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn on_menu<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let Some(core) = core(app) else { return };
    match id {
        "show" => show_main(app),
        "quit" => {
            core.manager.kill_all_now();
            app.exit(0);
        }
        "stop_all" => {
            std::thread::spawn(move || {
                let ids = core.manager.running_ids();
                let _ = core.manager.stop_many(&ids);
            });
        }
        _ => {
            if let Some(gid) = id.strip_prefix("gstart:") {
                if let Ok(ids) = core.group_ids(gid) {
                    if let Err(e) = core.launch(ids, false, true) {
                        notify(app, "그룹 시작 실패", &e);
                    }
                }
            } else if let Some(gid) = id.strip_prefix("gstop:") {
                let gid = gid.to_string();
                std::thread::spawn(move || {
                    if let Ok(ids) = core.group_ids(&gid) {
                        let _ = core.stop_ids(&ids);
                    }
                });
            } else if let Some(pid) = id.strip_prefix("pstart:") {
                if let Ok(ids) = core.preset_ids(pid) {
                    if let Err(e) = core.launch(ids, false, true) {
                        notify(app, "프리셋 시작 실패", &e);
                    }
                }
            }
        }
    }
}

/// 창 닫기: 설정에 따라 트레이로 숨긴다.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else { return };
    let Some(core) = core(window.app_handle()) else { return };
    if !core.settings.lock().unwrap().close_to_tray {
        return;
    }
    api.prevent_close();
    let _ = window.hide();
    static NOTIFIED: OnceLock<()> = OnceLock::new();
    if NOTIFIED.set(()).is_ok() {
        notify(
            window.app_handle(),
            "트레이에서 계속 실행 중입니다",
            "서비스는 계속 실행됩니다. 트레이 아이콘을 눌러 다시 열거나, 트레이 메뉴의 '종료'로 끝낼 수 있습니다.",
        );
    }
}

fn last_states() -> &'static Mutex<HashMap<String, RunState>> {
    static LAST: OnceLock<Mutex<HashMap<String, RunState>>> = OnceLock::new();
    LAST.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 상태 변화에 따른 알림과 트레이 툴팁 갱신
pub fn on_status<R: Runtime>(app: &AppHandle<R>, status: &ServiceStatus) {
    let prev = last_states().lock().unwrap().insert(status.id.clone(), status.state);
    if prev == Some(status.state) {
        return;
    }
    let Some(core) = core(app) else { return };
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let running = core.manager.statuses().iter().filter(|s| matches!(s.state, RunState::Running | RunState::Starting)).count();
        let tip = if running > 0 { format!("Services Dashboard · 실행 {running}") } else { "Services Dashboard".into() };
        let _ = tray.set_tooltip(Some(tip));
    }
    if !core.settings.lock().unwrap().notifications {
        return;
    }
    let name = core
        .workspace
        .lock()
        .unwrap()
        .as_ref()
        .ok()
        .and_then(|ws| ws.service(&status.id).map(|s| s.name.clone()))
        .unwrap_or_else(|| status.id.clone());
    let was_active = matches!(prev, Some(RunState::Starting | RunState::Running));
    if status.state == RunState::Failed && was_active {
        let code = status.exit_code.map(|c| format!("종료 코드 {c}")).unwrap_or_else(|| "비정상 종료".into());
        notify(app, &format!("✕ {name} 비정상 종료"), &code);
    } else if status.state == RunState::Running && prev == Some(RunState::Starting) {
        // 창을 보고 있을 때는 알리지 않는다
        let focused = app.get_webview_window("main").is_some_and(|w| w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false));
        if !focused {
            notify(app, &format!("✓ {name} 준비 완료"), status.urls.first().map(String::as_str).unwrap_or(""));
        }
    }
}

//! 앱 안에서 업데이트.
//!
//! GitHub 최신 릴리스의 `latest.json`을 읽어 새 버전이 있는지 보고, 설치본을 받아 서명(앱에 넣은 공개 키)을
//! 검증한 뒤 설치한다. Windows 설치 프로그램의 제약으로 설치를 시작하면 앱이 바로 끝나고, 설치가 끝나면
//! 설치 프로그램이 앱을 다시 켠다. 그래서 설치 전에 실행 중인 서비스를 먼저 멈춘다.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::commands::{AppCore, stop_everything};

/// 시험용: 이 환경변수가 있으면 설정의 주소 대신 여기서 latest.json을 읽는다. 서명 검증은 그대로 한다.
pub const ENDPOINT_ENV: &str = "SERVICES_DASHBOARD_UPDATE_URL";
/// 받는 중 진행률 이벤트
pub const EVENT_PROGRESS: &str = "update://progress";

const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

/// 확인한 업데이트와 받아 둔 설치본
#[derive(Default)]
pub struct UpdateState {
    pending: Mutex<Option<Update>>,
    bytes: Mutex<Option<Vec<u8>>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    /// 릴리스 노트 (latest.json의 notes)
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// 새 버전이 있으면 그 정보를, 이미 최신이면 None
#[tauri::command]
pub async fn update_check(app: AppHandle, state: State<'_, UpdateState>) -> Result<Option<UpdateInfo>, String> {
    let mut builder = app.updater_builder().timeout(CHECK_TIMEOUT);
    if let Ok(url) = std::env::var(ENDPOINT_ENV) {
        let url = url.parse().map_err(|e| format!("업데이트 주소가 잘못됐습니다: {e}"))?;
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| format!("업데이트를 확인하지 못했습니다: {e}"))?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
    });
    *state.pending.lock().unwrap() = update;
    *state.bytes.lock().unwrap() = None;
    Ok(info)
}

/// 설치본을 받고 서명을 검증한다. 진행률은 `update://progress` 이벤트로 보낸다.
#[tauri::command]
pub async fn update_download(app: AppHandle, state: State<'_, UpdateState>) -> Result<(), String> {
    let update = state.pending.lock().unwrap().clone().ok_or("확인된 업데이트가 없습니다")?;
    let mut downloaded = 0u64;
    let mut last = Instant::now() - Duration::from_secs(1);
    let bytes = update
        .download(
            |chunk, total| {
                downloaded += chunk as u64;
                if last.elapsed() >= Duration::from_millis(100) || Some(downloaded) == total {
                    last = Instant::now();
                    let _ = app.emit(EVENT_PROGRESS, Progress { downloaded, total });
                }
            },
            || {},
        )
        .await
        .map_err(|e| format!("업데이트를 받지 못했습니다: {e}"))?;
    *state.bytes.lock().unwrap() = Some(bytes);
    Ok(())
}

/// 실행 중인 서비스를 멈추고 설치를 시작한다. 성공하면 앱이 끝나고, 설치가 끝나면 다시 켜진다.
#[tauri::command]
pub async fn update_install(core: State<'_, Arc<AppCore>>, state: State<'_, UpdateState>) -> Result<(), String> {
    let update = state.pending.lock().unwrap().clone().ok_or("확인된 업데이트가 없습니다")?;
    let bytes = state.bytes.lock().unwrap().take().ok_or("업데이트를 먼저 받아야 합니다")?;
    let core = core.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _ = stop_everything(&core);
        core.files.flush();
        update.install(bytes).map_err(|e| format!("업데이트를 설치하지 못했습니다: {e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 업데이트 설정이 실제 릴리스를 가리키는지 (https, 최신 릴리스의 latest.json, 공개 키)
    #[test]
    fn updater_config_points_to_latest_release() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let updater = &conf["plugins"]["updater"];
        let endpoints = updater["endpoints"].as_array().unwrap();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(
            endpoints[0].as_str().unwrap(),
            "https://github.com/hulbo/ServicesDashboard/releases/latest/download/latest.json"
        );
        assert!(updater["pubkey"].as_str().unwrap().len() > 100);
        assert_eq!(updater["windows"]["installMode"], "passive");
        // 서명에 적힌 버전과 latest.json의 버전이 같아야 하고, 버전이 없는 서명은 받지 않는다
        assert_eq!(updater["requireSignedVersion"], true);
        assert_eq!(updater.get("dangerousInsecureTransportProtocol"), None);
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], true);
    }

    #[test]
    fn update_info_is_camel_case() {
        let info = UpdateInfo { version: "0.2.0".into(), current_version: "0.1.11".into(), notes: None };
        let v = serde_json::to_value(info).unwrap();
        assert_eq!(v["currentVersion"], "0.1.11");
    }
}

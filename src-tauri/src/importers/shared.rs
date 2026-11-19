//! 프로젝트 공유 파일 `services-dashboard.json` 읽기·쓰기.
//!
//! 팀이 저장소에 함께 둘 수 있도록 프로젝트 경로는 `${projectDir}`로 바꿔 저장한다.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::util::{read_text, to_slash};
use super::{Candidate, RawPreset, SourceKind};
use crate::model::{Group, Preset, Service};

pub const FILE_NAME: &str = "services-dashboard.json";
pub const PROJECT_DIR: &str = "${projectDir}";
const SCHEMA: &str = "https://raw.githubusercontent.com/hulbo/ServicesDashboard/main/schema/workspace.schema.json";

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SharedFile {
    #[serde(rename = "$schema", default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default)]
    pub groups: Vec<Group>,
    #[serde(default)]
    pub services: Vec<Service>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub presets: Vec<Preset>,
}

/// 문자열 값 안의 root 경로(대소문자·구분자 무시)를 바꾼다.
fn replace_root(v: &mut Value, from: &str, to: &str) {
    match v {
        Value::String(s) => {
            let norm = s.replace('\\', "/");
            let lower = norm.to_lowercase();
            let from_l = from.to_lowercase();
            if lower == from_l || lower.starts_with(&format!("{from_l}/")) {
                *s = format!("{to}{}", &norm[from.len()..]);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| replace_root(x, from, to)),
        Value::Object(o) => o.values_mut().for_each(|x| replace_root(x, from, to)),
        _ => {}
    }
}

pub fn collect(root: &Path, warnings: &mut Vec<String>) -> (Vec<Candidate>, Vec<RawPreset>) {
    let Some(text) = read_text(&root.join(FILE_NAME)) else { return (Vec::new(), Vec::new()) };
    let root_s = to_slash(root);
    let text = text.replace(PROJECT_DIR, &root_s);
    let file: SharedFile = match serde_json::from_str(&text) {
        Ok(f) => f,
        Err(e) => {
            warnings.push(format!("{FILE_NAME}을(를) 해석하지 못했습니다: {e}"));
            return (Vec::new(), Vec::new());
        }
    };
    let group_name = |id: &Option<String>| id.as_ref().and_then(|g| file.groups.iter().find(|x| &x.id == g)).map(|g| g.name.clone());
    let mut out = Vec::new();
    for svc in &file.services {
        let mut s = svc.clone();
        let group = group_name(&s.group);
        s.group = None;
        s.extra.remove("source");
        let mut c = Candidate::new(format!("shared:{}", svc.id), SourceKind::Shared, FILE_NAME.into(), s);
        c.group = group;
        c.recommended = true;
        out.push(c);
    }
    let presets = file
        .presets
        .iter()
        .map(|p| RawPreset {
            key: format!("shared-preset:{}", p.id),
            name: p.name.clone(),
            origin: FILE_NAME.into(),
            // 공유 파일의 프리셋은 서비스 ID로 가리킨다 → 후보 키로 바꾼다
            members: p.services.iter().map(|id| format!("shared:{id}")).collect(),
            source: SourceKind::Shared,
        })
        .collect();
    (out, presets)
}

/// 서비스·그룹·프리셋을 공유 파일로 쓴다. `root_values`에 있는 경로 표현(절대 경로, `${var:X}`)은 `${projectDir}`로 바꾼다.
pub fn export(root: &Path, services: &[Service], groups: &[Group], presets: &[Preset], root_values: &[String]) -> Result<std::path::PathBuf, String> {
    let ids: std::collections::HashSet<&str> = services.iter().map(|s| s.id.as_str()).collect();
    let used_groups: Vec<Group> = groups.iter().filter(|g| services.iter().any(|s| s.group.as_deref() == Some(&g.id))).cloned().collect();
    let presets: Vec<Preset> = presets
        .iter()
        .filter_map(|p| {
            let members: Vec<String> = p.services.iter().filter(|s| ids.contains(s.as_str())).cloned().collect();
            (!members.is_empty()).then(|| Preset { services: members, ..p.clone() })
        })
        .collect();
    let cleaned: Vec<Service> = services
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.extra.remove("source");
            // 다른 서비스를 가리키는 의존은 내보낸 범위 안의 것만 남긴다
            s.depends_on.retain(|d| ids.contains(d.as_str()));
            s
        })
        .collect();
    let file = SharedFile { schema: Some(SCHEMA.into()), groups: used_groups, services: cleaned, presets };
    let mut value = serde_json::to_value(&file).map_err(|e| e.to_string())?;
    let root_s = to_slash(root);
    replace_root(&mut value, &root_s, PROJECT_DIR);
    let mut text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    for v in root_values {
        text = text.replace(v.as_str(), PROJECT_DIR);
    }
    let path = root.join(FILE_NAME);
    std::fs::write(&path, text + "\n").map_err(|e| format!("파일을 쓸 수 없습니다: {} ({e})", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_project_dir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let root_s = to_slash(root);
        let svc = Service {
            id: "api".into(),
            name: "Api".into(),
            group: Some("g1".into()),
            cwd: Some(format!("{root_s}/apps/api")),
            jar: Some(crate::model::JarOpts { path: "${var:SHOP}/dist/app.jar".into(), ..Default::default() }),
            depends_on: vec!["db".into(), "outside".into()],
            ..Default::default()
        };
        let db = Service { id: "db".into(), name: "db".into(), cwd: Some("${var:SHOP}".into()), ..Default::default() };
        let groups = vec![Group { id: "g1".into(), name: "server".into(), ..Default::default() }, Group { id: "g2".into(), name: "unused".into(), ..Default::default() }];
        let presets = vec![Preset { id: "p".into(), name: "all".into(), services: vec!["api".into(), "x".into()], ..Default::default() }];
        let path = export(root, &[svc, db], &groups, &presets, &["${var:SHOP}".into()]).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"${projectDir}/apps/api\"") && text.contains("${projectDir}/dist/app.jar"));
        assert!(!text.contains(&root_s) && !text.contains("unused") && !text.contains("outside"));

        let mut warnings = Vec::new();
        let (cands, presets) = collect(root, &mut warnings);
        assert!(warnings.is_empty());
        assert_eq!(cands.len(), 2);
        assert_eq!(cands[0].service.cwd.as_deref(), Some(format!("{root_s}/apps/api").as_str()));
        assert_eq!(cands[0].group.as_deref(), Some("server"));
        assert_eq!(presets[0].members, vec!["shared:api"]);
    }
}

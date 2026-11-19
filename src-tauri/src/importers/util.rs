//! 가져오기 공통 도우미.

use std::path::{Path, PathBuf};

/// 스캔하지 않는 폴더
const SKIP_DIRS: &[&str] = &[
    "node_modules", ".git", ".svn", ".idea", ".gradle", ".vscode", ".mvn", "build", "dist", "target", "out", "bin",
    "coverage", ".next", ".nuxt", ".output", "vendor", ".cache", "tmp", "temp", "logs",
];

/// 한 모듈에서 메인 클래스를 찾을 때 읽는 소스 파일 최대 수
const MAX_SOURCE_FILES: usize = 6000;

pub fn to_slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// root 기준 상대 경로 (슬래시). root 밖이면 절대 경로.
pub fn rel(root: &Path, p: &Path) -> String {
    match p.strip_prefix(root) {
        Ok(r) => to_slash(r),
        Err(_) => to_slash(p),
    }
}

pub fn read_text(p: &Path) -> Option<String> {
    std::fs::read_to_string(p).ok()
}

/// 파일 이름 조건에 맞는 파일을 찾는다 (SKIP_DIRS는 건너뜀).
pub fn walk_files(root: &Path, max_depth: usize, mut matches: impl FnMut(&str) -> bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = rd.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if depth < max_depth && !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                    stack.push((e.path(), depth + 1));
                }
            } else if ft.is_file() && matches(&name) {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// IntelliJ 파라미터 문자열을 인자 목록으로 나눈다 (따옴표 묶음 지원, 백슬래시는 그대로).
pub fn split_args(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for ch in input.chars() {
        match quote {
            Some(q) if ch == q => quote = None,
            Some(_) => cur.push(ch),
            None if ch == '"' || ch == '\'' => {
                quote = Some(ch);
                has = true;
            }
            None if ch.is_whitespace() => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            None => {
                cur.push(ch);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

/// `@SpringBootApplication`이 붙은 클래스의 FQN 목록.
///
/// 메인 클래스는 보통 최상위 패키지에 있으므로 얕은 폴더부터 읽고,
/// 찾은 깊이까지만 확인한 뒤 멈춘다 (큰 모듈에서도 소스 전체를 읽지 않는다).
pub fn spring_main_classes(module_dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut read = 0usize;
    let mut level: Vec<PathBuf> = ["java", "kotlin"]
        .iter()
        .map(|l| module_dir.join("src").join("main").join(l))
        .filter(|d| d.is_dir())
        .collect();
    while !level.is_empty() && found.is_empty() && read < MAX_SOURCE_FILES {
        let mut next = Vec::new();
        for dir in &level {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            let mut entries: Vec<_> = rd.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let path = e.path();
                let Ok(ft) = e.file_type() else { continue };
                if ft.is_dir() {
                    next.push(path);
                    continue;
                }
                let name = e.file_name().to_string_lossy().into_owned();
                if !(name.ends_with(".java") || name.ends_with(".kt")) {
                    continue;
                }
                read += 1;
                if e.metadata().map(|m| m.len() > 512 * 1024).unwrap_or(true) {
                    continue;
                }
                let Some(text) = read_text(&path) else { continue };
                if !text.contains("@SpringBootApplication") {
                    continue;
                }
                let package = text
                    .lines()
                    .map(str::trim)
                    .find_map(|l| l.strip_prefix("package "))
                    .map(|p| p.trim_end_matches(';').trim().to_string());
                let class = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                found.push(match package {
                    Some(p) if !p.is_empty() => format!("{p}.{class}"),
                    _ => class,
                });
            }
        }
        level = next;
    }
    found.sort();
    found.dedup();
    found
}

/// `application-<프로파일>.yml` 파일 이름과 `on-profile:` 값에서 프로파일 후보를 모은다.
pub fn spring_profiles(module_dir: &Path) -> Vec<String> {
    let res = module_dir.join("src").join("main").join("resources");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&res) else { return out };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(rest) = name.strip_prefix("application-") {
            for ext in [".yml", ".yaml", ".properties"] {
                if let Some(p) = rest.strip_suffix(ext) {
                    out.push(p.to_string());
                }
            }
        } else if name == "application.yml" || name == "application.yaml" {
            if let Some(text) = read_text(&e.path()) {
                for line in text.lines() {
                    let t = line.trim();
                    if let Some(v) = t.strip_prefix("on-profile:") {
                        out.extend(v.split(',').map(|s| s.trim().trim_matches(['"', '\'']).to_string()).filter(|s| !s.is_empty() && !s.contains('!')));
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// 인자에서 포트를 찾는다 (`--server.port=8081`, `--port 5174`, `--port=5174`).
pub fn port_from_args(args: &[String]) -> Option<u16> {
    for (i, a) in args.iter().enumerate() {
        for prefix in ["--server.port=", "--port="] {
            if let Some(v) = a.strip_prefix(prefix) {
                if let Ok(p) = v.parse() {
                    return Some(p);
                }
            }
        }
        if a == "--port" {
            if let Some(p) = args.get(i + 1).and_then(|v| v.parse().ok()) {
                return Some(p);
            }
        }
    }
    None
}

/// 폴더 이름으로 워크스페이스 변수 이름을 만든다 (예: my-project → MY_PROJECT).
pub fn var_name_for(root: &Path) -> String {
    let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut v: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_uppercase() } else { '_' }).collect();
    v = v.trim_matches('_').to_string();
    while v.contains("__") {
        v = v.replace("__", "_");
    }
    if v.is_empty() || v.starts_with(|c: char| c.is_ascii_digit()) { format!("ROOT_{v}").trim_end_matches('_').to_string() } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_ports() {
        assert_eq!(split_args(r#"--a=1 "--b=two words" 'c "d"'"#), vec!["--a=1", "--b=two words", r#"c "d""#]);
        assert_eq!(split_args("  "), Vec::<String>::new());
        assert_eq!(port_from_args(&split_args("--x --server.port=8081")), Some(8081));
        assert_eq!(port_from_args(&split_args("--host 0.0.0.0 --port 5174")), Some(5174));
        assert_eq!(port_from_args(&split_args("--debug")), None);
    }

    #[test]
    fn var_names() {
        assert_eq!(var_name_for(Path::new("C:/Project/my-project")), "MY_PROJECT");
        assert_eq!(var_name_for(Path::new("C:/Project/Shop 2nd")), "SHOP_2ND");
        assert_eq!(var_name_for(Path::new("C:/Project/2024")), "ROOT_2024");
        assert_eq!(var_name_for(Path::new("C:/Project/한글")), "ROOT");
    }
}

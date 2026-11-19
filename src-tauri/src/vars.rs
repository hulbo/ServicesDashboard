//! `${...}` 변수 치환.
//!
//! - `${var:NAME}`  워크스페이스 변수 (다른 변수를 참조할 수 있다)
//! - `${env:NAME}`  대시보드 프로세스의 환경변수 (없으면 빈 문자열)
//! - `${userHome}`  사용자 홈 디렉터리
//! - `${workspaceDir}` 워크스페이스 파일이 있는 디렉터리

use crate::model::EnvMap;

const MAX_DEPTH: usize = 8;

pub struct VarContext {
    vars: EnvMap,
    user_home: String,
    workspace_dir: String,
}

impl VarContext {
    pub fn new(vars: &EnvMap, workspace_dir: &str) -> Self {
        let user_home = dirs::home_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        VarContext {
            vars: vars.clone(),
            user_home,
            workspace_dir: workspace_dir.to_string(),
        }
    }

    pub fn expand(&self, input: &str) -> Result<String, String> {
        self.expand_depth(input, 0)
    }

    pub fn expand_opt(&self, input: Option<&String>) -> Result<Option<String>, String> {
        match input {
            Some(s) if !s.trim().is_empty() => Ok(Some(self.expand(s.trim())?)),
            _ => Ok(None),
        }
    }

    pub fn expand_all(&self, items: &[String]) -> Result<Vec<String>, String> {
        items.iter().map(|s| self.expand(s)).collect()
    }

    fn expand_depth(&self, input: &str, depth: usize) -> Result<String, String> {
        if depth > MAX_DEPTH {
            return Err(format!("변수가 너무 깊게(순환) 참조됩니다: {input}"));
        }
        let mut out = String::with_capacity(input.len());
        let mut rest = input;
        while let Some(start) = rest.find("${") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find('}') else {
                return Err(format!("닫히지 않은 변수 표현식이 있습니다: {input}"));
            };
            let key = &after[..end];
            out.push_str(&self.lookup(key, depth)?);
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        Ok(out)
    }

    fn lookup(&self, key: &str, depth: usize) -> Result<String, String> {
        if let Some(name) = key.strip_prefix("var:") {
            let raw = self
                .vars
                .get(name)
                .ok_or_else(|| format!("워크스페이스 변수 '{name}'가 정의돼 있지 않습니다."))?;
            return self.expand_depth(raw, depth + 1);
        }
        if let Some(name) = key.strip_prefix("env:") {
            return Ok(std::env::var(name).unwrap_or_default());
        }
        match key {
            "userHome" => Ok(self.user_home.clone()),
            "workspaceDir" => Ok(self.workspace_dir.clone()),
            _ => Err(format!("알 수 없는 변수입니다: ${{{key}}}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> VarContext {
        let mut vars = EnvMap::new();
        vars.insert("ROOT".into(), "C:/proj".into());
        vars.insert("FRONT".into(), "${var:ROOT}/front".into());
        vars.insert("LOOP".into(), "${var:LOOP}".into());
        VarContext::new(&vars, "C:/ws")
    }

    #[test]
    fn expands_nested_vars() {
        let c = ctx();
        assert_eq!(c.expand("${var:FRONT}/portal").unwrap(), "C:/proj/front/portal");
        assert_eq!(c.expand("${workspaceDir}/x").unwrap(), "C:/ws/x");
        assert_eq!(c.expand("plain").unwrap(), "plain");
    }

    #[test]
    fn reports_errors() {
        let c = ctx();
        assert!(c.expand("${var:NOPE}").is_err());
        assert!(c.expand("${var:LOOP}").unwrap_err().contains("순환"));
        assert!(c.expand("${oops}").is_err());
        assert!(c.expand("${var:ROOT").is_err());
    }

    #[test]
    fn missing_env_is_empty() {
        let c = ctx();
        assert_eq!(c.expand("[${env:SD_TEST_SURELY_MISSING_123}]").unwrap(), "[]");
    }
}

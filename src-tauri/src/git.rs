//! 서비스가 쓰는 git 저장소: 현재 브랜치 읽기, 브랜치 목록, 전환, 원격 가져오기.
//!
//! 현재 브랜치는 `.git/HEAD` 파일만 읽어 가볍게 확인한다 (화면이 몇 초마다 묻는다).
//! 목록·전환·fetch는 PC에 설치된 git 명령을 창 없이 실행한다. 커밋·push는 하지 않는다.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

/// 목록·상태 확인처럼 금방 끝나는 명령의 제한 시간
const QUICK: Duration = Duration::from_secs(20);
/// fetch·pull처럼 네트워크를 쓰는 명령의 제한 시간
const NETWORK: Duration = Duration::from_secs(180);
/// 화면에 보여 줄 변경 파일 수 한도
const MAX_CHANGES: usize = 200;

/// 작업 트리 루트와 git 디렉터리
#[derive(Debug, Clone, PartialEq)]
pub struct RepoLocation {
    pub root: PathBuf,
    pub git_dir: PathBuf,
}

/// `start`에서 위로 올라가며 저장소를 찾는다. `.git`이 파일(worktree·submodule)이면 `gitdir:`을 따라간다.
pub fn find_repo(start: &Path) -> Option<RepoLocation> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        let dot = d.join(".git");
        if dot.is_dir() {
            return Some(RepoLocation { root: d.to_path_buf(), git_dir: dot });
        }
        if dot.is_file() {
            let text = std::fs::read_to_string(&dot).ok()?;
            let target = text.lines().find_map(|l| l.trim().strip_prefix("gitdir:"))?.trim();
            let p = PathBuf::from(target);
            let git_dir = if p.is_absolute() { p } else { d.join(p) };
            return Some(RepoLocation { root: d.to_path_buf(), git_dir });
        }
        dir = d.parent();
    }
    None
}

/// HEAD가 가리키는 것: 브랜치, 또는 브랜치가 아닌 커밋(detached)
#[derive(Debug, Clone, PartialEq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Head {
    pub branch: Option<String>,
    /// detached HEAD일 때 짧은 커밋 해시
    pub commit: Option<String>,
}

pub fn parse_head(text: &str) -> Head {
    let t = text.trim();
    if let Some(r) = t.strip_prefix("ref:") {
        let r = r.trim();
        return Head { branch: Some(r.strip_prefix("refs/heads/").unwrap_or(r).to_string()), commit: None };
    }
    Head { branch: None, commit: (!t.is_empty()).then(|| t.chars().take(8).collect()) }
}

pub fn read_head(git_dir: &Path) -> Head {
    std::fs::read_to_string(git_dir.join("HEAD")).map(|t| parse_head(&t)).unwrap_or_default()
}

/// 경로 비교용 (역슬래시·끝 슬래시·대소문자 무시)
pub fn norm(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_lowercase()
}

pub fn display(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    // `C:/` 같은 드라이브 루트는 그대로 둔다
    if s.len() > 3 { s.trim_end_matches('/').to_string() } else { s }
}

/// PATH와 기본 설치 위치에서 git 실행 파일을 찾는다.
pub fn find_git() -> Result<PathBuf, String> {
    let exe = if cfg!(windows) { "git.exe" } else { "git" };
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(exe);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    #[cfg(windows)]
    for (var, rel) in [
        ("ProgramFiles", r"Git\cmd\git.exe"),
        ("ProgramW6432", r"Git\cmd\git.exe"),
        ("ProgramFiles(x86)", r"Git\cmd\git.exe"),
        ("LOCALAPPDATA", r"Programs\Git\cmd\git.exe"),
    ] {
        if let Some(base) = std::env::var_os(var) {
            let p = PathBuf::from(base).join(rel);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err("git을 찾지 못했습니다. Git for Windows를 설치하거나 PATH에 git을 추가하세요.".into())
}

pub struct Git {
    exe: PathBuf,
    root: PathBuf,
}

impl Git {
    pub fn open(root: &Path) -> Result<Self, String> {
        let repo = find_repo(root).ok_or_else(|| format!("git 저장소가 아닙니다: {}", display(root)))?;
        Ok(Git { exe: find_git()?, root: repo.root })
    }

    /// git을 창 없이 실행하고 표준 출력을 돌려준다. 실패하면 git이 남긴 메시지를 오류로 돌려준다.
    fn run(&self, args: &[&str], timeout: Duration) -> Result<String, String> {
        let mut cmd = Command::new(&self.exe);
        // 한글 파일 이름을 \353... 같은 이스케이프 없이 받는다
        cmd.args(["-c", "core.quotepath=false", "-c", "color.ui=never"])
            .args(args)
            .current_dir(&self.root)
            // 인증 정보를 터미널에서 묻다가 멈추지 않게 한다 (자격 증명 관리자 창은 그대로 뜬다)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| format!("git을 실행하지 못했습니다: {e}"))?;
        let mut out = child.stdout.take().unwrap();
        let mut err = child.stderr.take().unwrap();
        let out_t = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = out.read_to_end(&mut b);
            b
        });
        let err_t = std::thread::spawn(move || {
            let mut b = Vec::new();
            let _ = err.read_to_end(&mut b);
            b
        });
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break s,
                Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
                Ok(None) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("git {}이(가) {}초 안에 끝나지 않아 중단했습니다.", args.join(" "), timeout.as_secs()));
                }
                Err(e) => return Err(e.to_string()),
            }
        };
        let stdout = String::from_utf8_lossy(&out_t.join().unwrap_or_default()).into_owned();
        let stderr = String::from_utf8_lossy(&err_t.join().unwrap_or_default()).trim().to_string();
        if status.success() {
            Ok(stdout)
        } else {
            let msg = if stderr.is_empty() { stdout.trim().to_string() } else { stderr };
            Err(if msg.is_empty() { format!("git {} 실패 ({status})", args.join(" ")) } else { msg })
        }
    }

    /// 커밋하지 않은 변경 (추적 중인 파일만. 새 파일은 전환을 막지 않는다)
    pub fn changes(&self) -> Result<Vec<String>, String> {
        let out = self.run(&["status", "--porcelain=v1", "--untracked-files=no"], QUICK)?;
        Ok(out.lines().filter(|l| !l.trim().is_empty()).take(MAX_CHANGES).map(|l| l.trim_end().to_string()).collect())
    }

    fn locals(&self) -> Result<Vec<LocalBranch>, String> {
        let out = self.run(&["for-each-ref", "--sort=-committerdate", "--format=%(refname:short)%09%(upstream:short)", "refs/heads"], QUICK)?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let mut it = l.split('\t');
                let name = it.next()?.trim();
                let upstream = it.next().map(str::trim).filter(|u| !u.is_empty()).map(String::from);
                (!name.is_empty()).then(|| LocalBranch { name: name.to_string(), upstream })
            })
            .collect())
    }

    fn remotes(&self) -> Result<Vec<String>, String> {
        let out = self.run(&["for-each-ref", "--sort=-committerdate", "--format=%(refname:short)%09%(symref)", "refs/remotes"], QUICK)?;
        Ok(out
            .lines()
            .filter_map(|l| {
                let mut it = l.split('\t');
                let name = it.next()?.trim();
                // origin/HEAD 같은 별칭은 뺀다
                let symref = it.next().map(str::trim).unwrap_or("");
                (symref.is_empty() && name.contains('/')).then(|| name.to_string())
            })
            .collect())
    }

    pub fn branches(&self) -> Result<BranchInfo, String> {
        let repo = find_repo(&self.root).ok_or("git 저장소를 찾지 못했습니다.")?;
        let head = read_head(&repo.git_dir);
        let locals = self.locals()?;
        // 로컬 브랜치가 이미 있거나 추적 중인 원격 브랜치는 따로 보여 주지 않는다
        let remotes = self
            .remotes()?
            .into_iter()
            .filter(|r| !locals.iter().any(|l| l.upstream.as_deref() == Some(r) || Some(l.name.as_str()) == local_name(r)))
            .collect();
        let (mut ahead, mut behind) = (None, None);
        if let Some(cur) = locals.iter().find(|l| Some(&l.name) == head.branch.as_ref()) {
            if cur.upstream.is_some() {
                if let Ok(out) = self.run(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"], QUICK) {
                    let mut it = out.split_whitespace().filter_map(|n| n.parse::<u32>().ok());
                    ahead = it.next();
                    behind = it.next();
                }
            }
        }
        Ok(BranchInfo { root: display(&self.root), head, locals, remotes, changes: self.changes()?, ahead, behind })
    }

    pub fn fetch(&self) -> Result<(), String> {
        self.run(&["fetch", "--all", "--prune"], NETWORK).map(|_| ())
    }

    /// 브랜치를 전환한다. 원격 브랜치(`origin/x`)는 같은 이름의 로컬 브랜치를 만들어 추적한다.
    /// 커밋하지 않은 변경이 있으면 전환하지 않는다 (자동 stash는 하지 않는다).
    pub fn switch(&self, target: &str) -> Result<String, String> {
        let target = target.trim();
        if target.is_empty() || target.starts_with('-') {
            return Err(format!("브랜치 이름이 올바르지 않습니다: {target}"));
        }
        self.ensure_clean()?;
        let locals = self.locals()?;
        if locals.iter().any(|l| l.name == target) {
            self.run(&["switch", target], QUICK)?;
            return Ok(target.to_string());
        }
        if self.remotes()?.iter().any(|r| r == target) {
            let local = local_name(target).ok_or_else(|| format!("브랜치 이름이 올바르지 않습니다: {target}"))?;
            if locals.iter().any(|l| l.name == local) {
                self.run(&["switch", local], QUICK)?;
            } else {
                self.run(&["switch", "--track", target], QUICK)?;
            }
            return Ok(local.to_string());
        }
        Err(format!("브랜치를 찾을 수 없습니다: {target} (원격 브랜치라면 먼저 '원격 가져오기'를 눌러 보세요)"))
    }

    /// 현재 브랜치를 원격과 맞춘다 (fast-forward만. 병합 커밋을 만들지 않는다)
    pub fn pull(&self) -> Result<(), String> {
        self.run(&["pull", "--ff-only"], NETWORK).map(|_| ())
    }

    pub fn ensure_clean(&self) -> Result<(), String> {
        let changes = self.changes()?;
        if changes.is_empty() {
            return Ok(());
        }
        let shown: Vec<&str> = changes.iter().take(5).map(|c| c.get(3..).unwrap_or(c).trim()).collect();
        let more = if changes.len() > shown.len() { format!(" 외 {}개", changes.len() - shown.len()) } else { String::new() };
        Err(format!(
            "커밋하지 않은 변경이 {}개 있어 전환하지 않았습니다: {}{more}. IntelliJ나 git에서 커밋하거나 stash한 뒤 다시 시도하세요.",
            changes.len(),
            shown.join(", ")
        ))
    }
}

/// `origin/feature/x` → `feature/x`
fn local_name(remote: &str) -> Option<&str> {
    remote.split_once('/').map(|(_, b)| b).filter(|b| !b.is_empty())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LocalBranch {
    pub name: String,
    pub upstream: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub root: String,
    pub head: Head,
    /// 최근 커밋 순
    pub locals: Vec<LocalBranch>,
    /// 로컬 브랜치가 없는 원격 브랜치 (최근 커밋 순)
    pub remotes: Vec<String>,
    /// 커밋하지 않은 변경 (`git status --porcelain` 줄)
    pub changes: Vec<String>,
    /// 현재 브랜치가 원격보다 앞선·뒤처진 커밋 수
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_parsing() {
        assert_eq!(parse_head("ref: refs/heads/feature/login\n"), Head { branch: Some("feature/login".into()), commit: None });
        assert_eq!(parse_head("3f2a9c1d0e5b7a8c9d0e1f2a3b4c5d6e7f8a9b0c\n"), Head { branch: None, commit: Some("3f2a9c1d".into()) });
        assert_eq!(parse_head(""), Head::default());
        assert_eq!(local_name("origin/feature/x"), Some("feature/x"));
        assert_eq!(local_name("origin"), None);
    }

    #[test]
    fn finds_repo_from_subdir_and_worktree_file() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        std::fs::create_dir_all(root.join("apps/api")).unwrap();
        let r = find_repo(&root.join("apps/api")).unwrap();
        assert_eq!(r.root, root);
        assert_eq!(read_head(&r.git_dir).branch.as_deref(), Some("main"));

        // worktree: .git 파일이 실제 git 디렉터리를 가리킨다
        let wt = tmp.path().join("wt");
        let wt_git = root.join(".git/worktrees/wt");
        std::fs::create_dir_all(&wt_git).unwrap();
        std::fs::write(wt_git.join("HEAD"), "ref: refs/heads/hotfix\n").unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(wt.join(".git"), format!("gitdir: {}\n", wt_git.display())).unwrap();
        let r = find_repo(&wt).unwrap();
        assert_eq!(r.root, wt);
        assert_eq!(read_head(&r.git_dir).branch.as_deref(), Some("hotfix"));

        assert!(find_repo(tmp.path()).is_none());
        assert_eq!(norm(Path::new("C:\\A\\b\\")), "c:/a/b");
    }

    /// 실제 git으로 목록·전환·원격 브랜치 추적·변경 있을 때 거부를 확인한다 (git이 없으면 건너뜀)
    #[test]
    fn switch_with_real_git() {
        let Ok(exe) = find_git() else {
            eprintln!("git이 없어 건너뜀");
            return;
        };
        let tmp = tempfile::tempdir().unwrap();
        let sh = |dir: &Path, args: &[&str]| {
            let mut c = Command::new(&exe);
            c.args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "init.defaultBranch=main", "-c", "commit.gpgsign=false"]).args(args).current_dir(dir);
            let out = c.output().unwrap();
            assert!(out.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&out.stderr));
        };
        // 원격 역할 저장소
        let upstream = tmp.path().join("upstream");
        std::fs::create_dir_all(&upstream).unwrap();
        sh(&upstream, &["init", "-q"]);
        std::fs::write(upstream.join("app.txt"), "main\n").unwrap();
        sh(&upstream, &["add", "."]);
        sh(&upstream, &["commit", "-q", "-m", "init"]);
        sh(&upstream, &["switch", "-q", "-c", "feature/한글"]);
        std::fs::write(upstream.join("app.txt"), "feature\n").unwrap();
        sh(&upstream, &["commit", "-q", "-am", "feature"]);
        sh(&upstream, &["switch", "-q", "main"]);

        let work = tmp.path().join("work");
        sh(tmp.path(), &["clone", "-q", upstream.to_str().unwrap(), "work"]);
        sh(&work, &["branch", "local-only"]);

        let git = Git::open(&work.join(".")).unwrap();
        let info = git.branches().unwrap();
        assert_eq!(info.head.branch.as_deref(), Some("main"));
        assert!(info.locals.iter().any(|l| l.name == "main" && l.upstream.as_deref() == Some("origin/main")));
        assert!(info.locals.iter().any(|l| l.name == "local-only"));
        assert_eq!(info.remotes, vec!["origin/feature/한글".to_string()], "로컬이 추적 중인 origin/main은 빠진다");
        assert_eq!((info.ahead, info.behind), (Some(0), Some(0)));
        assert!(info.changes.is_empty());

        // 원격 브랜치 → 같은 이름의 로컬 브랜치를 만들어 추적
        assert_eq!(git.switch("origin/feature/한글").unwrap(), "feature/한글");
        // core.autocrlf가 켜진 PC에서는 줄바꿈이 \r\n이 된다
        assert_eq!(std::fs::read_to_string(work.join("app.txt")).unwrap().trim(), "feature");
        let info = git.branches().unwrap();
        assert_eq!(info.head.branch.as_deref(), Some("feature/한글"));
        assert!(info.remotes.is_empty());

        // 커밋하지 않은 변경이 있으면 거부하고 브랜치를 그대로 둔다
        std::fs::write(work.join("app.txt"), "dirty\n").unwrap();
        let err = git.switch("main").unwrap_err();
        assert!(err.contains("커밋하지 않은 변경이 1개") && err.contains("app.txt"), "{err}");
        assert_eq!(git.branches().unwrap().head.branch.as_deref(), Some("feature/한글"));
        sh(&work, &["checkout", "--", "app.txt"]);

        // 새 파일(추적 안 함)은 전환을 막지 않는다
        std::fs::write(work.join("note.txt"), "memo\n").unwrap();
        assert_eq!(git.switch("main").unwrap(), "main");
        assert!(git.switch("--orphan").is_err());
        assert!(git.switch("nope").unwrap_err().contains("찾을 수 없습니다"));

        // 원격이 앞서면 behind가 보이고 pull --ff-only로 따라간다
        std::fs::write(upstream.join("app.txt"), "main2\n").unwrap();
        sh(&upstream, &["commit", "-q", "-am", "main2"]);
        git.fetch().unwrap();
        let info = git.branches().unwrap();
        assert_eq!((info.ahead, info.behind), (Some(0), Some(1)));
        git.pull().unwrap();
        assert_eq!(std::fs::read_to_string(work.join("app.txt")).unwrap().trim(), "main2");
    }
}

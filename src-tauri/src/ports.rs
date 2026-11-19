//! 포트를 점유한 프로세스 찾기와 종료.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PortOwner {
    pub port: u16,
    pub pid: u32,
    /// 실행 파일 이름 (예: java.exe)
    pub name: String,
    /// 이 대시보드에서 실행 중인 서비스가 점유한 경우 그 서비스 ID
    pub service_id: Option<String>,
}

#[cfg(windows)]
mod imp {
    use super::PortOwner;
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
    use windows_sys::Win32::NetworkManagement::IpHelper::{GetExtendedTcpTable, TCP_TABLE_OWNER_PID_LISTENER};
    use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };

    /// (포트, PID) 목록 — LISTEN 상태만
    fn listeners(family: u32) -> Vec<(u16, u32)> {
        unsafe {
            let mut size: u32 = 0;
            let rc = GetExtendedTcpTable(std::ptr::null_mut(), &mut size, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0);
            if rc != ERROR_INSUFFICIENT_BUFFER || size == 0 {
                return Vec::new();
            }
            let mut buf = vec![0u8; size as usize + 1024];
            size = buf.len() as u32;
            if GetExtendedTcpTable(buf.as_mut_ptr().cast(), &mut size, 0, family, TCP_TABLE_OWNER_PID_LISTENER, 0) != NO_ERROR {
                return Vec::new();
            }
            let read_u32 = |off: usize| -> u32 {
                if off + 4 > buf.len() { 0 } else { u32::from_ne_bytes(buf[off..off + 4].try_into().unwrap()) }
            };
            let count = read_u32(0) as usize;
            // MIB_TCPROW_OWNER_PID: 6 × u32 (state, localAddr, localPort, remoteAddr, remotePort, pid)
            // MIB_TCP6ROW_OWNER_PID: localAddr[16], scope, localPort, remoteAddr[16], scope, remotePort, state, pid
            let (row, port_off, pid_off) = if family == AF_INET as u32 { (24, 8, 20) } else { (56, 20, 52) };
            (0..count)
                .map(|i| 4 + i * row)
                .filter(|base| base + row <= buf.len())
                .map(|base| {
                    let raw = read_u32(base + port_off) as u16;
                    (u16::from_be(raw), read_u32(base + pid_off))
                })
                .collect()
        }
    }

    pub fn process_name(pid: u32) -> String {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return format!("PID {pid}");
            }
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
            CloseHandle(h);
            if ok == 0 {
                return format!("PID {pid}");
            }
            let full = String::from_utf16_lossy(&buf[..len as usize]);
            full.rsplit(['\\', '/']).next().unwrap_or(&full).to_string()
        }
    }

    pub fn owners(ports: &[u16]) -> Vec<PortOwner> {
        let mut out: Vec<PortOwner> = Vec::new();
        for family in [AF_INET as u32, AF_INET6 as u32] {
            for (port, pid) in listeners(family) {
                if ports.contains(&port) && pid != 0 && !out.iter().any(|o| o.port == port && o.pid == pid) {
                    out.push(PortOwner { port, pid, name: process_name(pid), service_id: None });
                }
            }
        }
        out.sort_by_key(|o| (o.port, o.pid));
        out
    }
}

#[cfg(not(windows))]
mod imp {
    use super::PortOwner;
    pub fn owners(_ports: &[u16]) -> Vec<PortOwner> {
        Vec::new()
    }
}

pub use imp::owners;

/// 프로세스 트리를 강제 종료한다 (`taskkill /T /F`).
pub fn kill_tree(pid: u32) -> Result<(), String> {
    if pid == std::process::id() {
        return Err("대시보드 자신은 종료할 수 없습니다.".into());
    }
    let mut cmd = std::process::Command::new(if cfg!(windows) { "taskkill" } else { "kill" });
    if cfg!(windows) {
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"]);
    } else {
        cmd.args(["-9", &pid.to_string()]);
    }
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
    }
    let status = cmd.status().map_err(|e| format!("프로세스를 종료할 수 없습니다: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("PID {pid}를 종료하지 못했습니다 (권한이 없거나 이미 종료됨).")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn finds_own_listener() {
        let l4 = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l4.local_addr().unwrap().port();
        let found = owners(&[port, 1]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].pid, std::process::id());
        assert!(found[0].name.to_lowercase().ends_with(".exe"), "{}", found[0].name);
        drop(l4);
        assert!(owners(&[port]).is_empty());

        let l6 = std::net::TcpListener::bind("[::1]:0").unwrap();
        let port6 = l6.local_addr().unwrap().port();
        assert_eq!(owners(&[port6]).len(), 1, "IPv6 리스너도 찾는다");
    }

    #[test]
    fn refuses_to_kill_self() {
        assert!(kill_tree(std::process::id()).unwrap_err().contains("자신"));
    }
}

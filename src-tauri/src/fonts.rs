//! 설치된 글꼴 목록 (설정 → 글꼴에서 고른다).
//!
//! Windows DirectWrite의 시스템 글꼴 모음을 읽는다. 레지스트리의 파일 이름과 달리
//! 실제 글꼴 패밀리 이름(굵기·기울임 변형은 한 패밀리로)과 한국어 이름, 고정폭 여부를 얻을 수 있다.

use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FontFamily {
    /// CSS `font-family`에 쓰는 이름 (영어 이름, 없으면 첫 이름)
    pub name: String,
    /// 한국어 이름이 따로 있으면 (예: Malgun Gothic → 맑은 고딕)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    /// 고정폭 글꼴 (콘솔용으로 위에 보여 준다)
    pub mono: bool,
}

/// 이름순(대소문자 무시), 같은 이름은 하나만.
fn tidy(mut list: Vec<FontFamily>) -> Vec<FontFamily> {
    list.retain(|f| !f.name.trim().is_empty() && !f.name.starts_with('@'));
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.name.cmp(&b.name)));
    list.dedup_by(|a, b| a.name.eq_ignore_ascii_case(&b.name));
    list
}

#[cfg(windows)]
pub fn system_fonts() -> Result<Vec<FontFamily>, String> {
    use windows::Win32::Graphics::DirectWrite::*;
    use windows::core::{BOOL, Interface, w};

    unsafe fn name_at(names: &IDWriteLocalizedStrings, locale: windows::core::PCWSTR) -> Option<String> {
        let mut index = 0u32;
        let mut exists = BOOL(0);
        unsafe { names.FindLocaleName(locale, &mut index, &mut exists) }.ok()?;
        if !exists.as_bool() {
            return None;
        }
        unsafe { string_at(names, index) }
    }

    unsafe fn string_at(names: &IDWriteLocalizedStrings, index: u32) -> Option<String> {
        let len = unsafe { names.GetStringLength(index) }.ok()? as usize;
        let mut buf = vec![0u16; len + 1];
        unsafe { names.GetString(index, &mut buf) }.ok()?;
        Some(String::from_utf16_lossy(&buf[..len]))
    }

    let err = |e: windows::core::Error| format!("글꼴 목록을 읽지 못했습니다: {e}");
    let mut list = Vec::new();
    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).map_err(err)?;
        let mut collection = None;
        factory.GetSystemFontCollection(&mut collection, false).map_err(err)?;
        let Some(collection) = collection else { return Ok(list) };
        for i in 0..collection.GetFontFamilyCount() {
            let Ok(family) = collection.GetFontFamily(i) else { continue };
            let Ok(names) = family.GetFamilyNames() else { continue };
            let Some(name) = name_at(&names, w!("en-us")).or_else(|| string_at(&names, 0)) else { continue };
            let local = name_at(&names, w!("ko-kr")).filter(|k| k != &name);
            let mono = family
                .GetFirstMatchingFont(DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL)
                .ok()
                .and_then(|f| f.cast::<IDWriteFont1>().ok())
                .is_some_and(|f| f.IsMonospacedFont().as_bool());
            list.push(FontFamily { name, local, mono });
        }
    }
    Ok(tidy(list))
}

#[cfg(not(windows))]
pub fn system_fonts() -> Result<Vec<FontFamily>, String> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidy_sorts_and_dedups() {
        let f = |name: &str| FontFamily { name: name.into(), local: None, mono: false };
        let list = tidy(vec![f("consolas"), f("Arial"), f("@Malgun Gothic"), f(""), f("Consolas"), f("Batang")]);
        let names: Vec<_> = list.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Arial", "Batang", "Consolas"]);
    }

    /// 실제 Windows 글꼴 목록: 기본 글꼴이 있고, Consolas는 고정폭, 맑은 고딕은 한국어 이름이 있다.
    #[cfg(windows)]
    #[test]
    fn reads_windows_fonts() {
        let list = system_fonts().unwrap();
        assert!(list.len() > 10, "{}", list.len());
        let consolas = list.iter().find(|f| f.name == "Consolas").expect("Consolas");
        assert!(consolas.mono);
        let arial = list.iter().find(|f| f.name == "Arial").expect("Arial");
        assert!(!arial.mono);
        if let Some(malgun) = list.iter().find(|f| f.name == "Malgun Gothic") {
            assert_eq!(malgun.local.as_deref(), Some("맑은 고딕"));
        }
    }
}

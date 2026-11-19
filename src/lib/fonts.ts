import type { FontFamily, FontSettings } from './types'

export const DEFAULT_FONTS: FontSettings = { ui: '', mono: '', consoleSize: 12.5 }

/** 콘솔 글자 크기 선택지 (px) */
export const CONSOLE_SIZES = [10, 11, 12, 12.5, 13, 14, 15, 16, 18, 20]

/** 설정에 글꼴 항목이 없던 예전 값도 받아 준다. */
export function fontsOf(settings: { fonts?: Partial<FontSettings> } | null | undefined): FontSettings {
  return { ...DEFAULT_FONTS, ...(settings?.fonts ?? {}) }
}

/** CSS font-family 값으로 쓸 수 있게 따옴표로 감싼다. 빈 이름은 null (기본 글꼴). */
export function cssFamily(name: string): string | null {
  const n = name.trim()
  if (!n) return null
  return `"${n.replace(/["\\]/g, '\\$&')}"`
}

/** 콘솔 한 줄 높이 (px). 가상 스크롤이 이 높이로 줄 위치를 계산한다. 기본 12.5px → 18px. */
export function consoleRow(size: number): number {
  return Math.max(12, Math.round(size * 1.44))
}

/**
 * 문서에 글꼴을 적용한다. app.css의 --font-ui·--font-mono가 --user-font-* 를 맨 앞에 두므로,
 * 고른 글꼴에 없는 글자(한글 등)는 기본 글꼴로 이어진다.
 */
export function applyFonts(f: FontSettings, root: HTMLElement = document.documentElement) {
  const set = (prop: string, value: string | null) => (value ? root.style.setProperty(prop, value) : root.style.removeProperty(prop))
  set('--user-font-ui', cssFamily(f.ui))
  set('--user-font-mono', cssFamily(f.mono))
  set('--console-font-size', `${f.consoleSize}px`)
  set('--console-row', `${consoleRow(f.consoleSize)}px`)
}

/** 목록에 보일 이름: 한국어 이름이 있으면 "맑은 고딕 (Malgun Gothic)" */
export function fontLabel(f: FontFamily): string {
  return f.local ? `${f.local} (${f.name})` : f.name
}

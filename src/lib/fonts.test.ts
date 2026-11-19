import { describe, expect, it } from 'vitest'
import { applyFonts, consoleRow, cssFamily, DEFAULT_FONTS, fontLabel, fontsOf } from './fonts'

describe('글꼴', () => {
  it('CSS 이름: 따옴표로 감싸고, 비어 있으면 기본 글꼴', () => {
    expect(cssFamily('D2Coding')).toBe('"D2Coding"')
    expect(cssFamily('  맑은 고딕 ')).toBe('"맑은 고딕"')
    expect(cssFamily('a"b\\c')).toBe('"a\\"b\\\\c"')
    expect(cssFamily('')).toBeNull()
    expect(cssFamily('   ')).toBeNull()
  })

  it('콘솔 줄 높이는 글자 크기를 따른다 (기본 12.5px → 18px)', () => {
    expect(consoleRow(12.5)).toBe(18)
    expect(consoleRow(14)).toBe(20)
    expect(consoleRow(20)).toBe(29)
    expect(consoleRow(5)).toBe(12)
  })

  it('예전 설정(글꼴 항목 없음)은 기본값', () => {
    expect(fontsOf({})).toEqual(DEFAULT_FONTS)
    expect(fontsOf(null)).toEqual(DEFAULT_FONTS)
    expect(fontsOf({ fonts: { mono: 'Consolas' } })).toEqual({ ...DEFAULT_FONTS, mono: 'Consolas' })
  })

  it('문서에 적용하고, 기본으로 돌리면 지운다', () => {
    const props = new Map<string, string>()
    const root = { style: { setProperty: (k: string, v: string) => props.set(k, v), removeProperty: (k: string) => props.delete(k) } } as unknown as HTMLElement
    applyFonts({ ui: 'Malgun Gothic', mono: 'D2Coding', consoleSize: 14 }, root)
    expect(Object.fromEntries(props)).toEqual({
      '--user-font-ui': '"Malgun Gothic"',
      '--user-font-mono': '"D2Coding"',
      '--console-font-size': '14px',
      '--console-row': '20px',
    })
    applyFonts(DEFAULT_FONTS, root)
    expect(props.has('--user-font-ui')).toBe(false)
    expect(props.has('--user-font-mono')).toBe(false)
    expect(props.get('--console-row')).toBe('18px')
  })

  it('목록 이름: 한국어 이름이 있으면 함께', () => {
    expect(fontLabel({ name: 'Malgun Gothic', local: '맑은 고딕', mono: false })).toBe('맑은 고딕 (Malgun Gothic)')
    expect(fontLabel({ name: 'Consolas', mono: true })).toBe('Consolas')
  })
})

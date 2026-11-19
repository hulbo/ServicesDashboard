import { describe, expect, it } from 'vitest'
import { joinArgs, parsePorts, splitArgs, splitList } from './args'

describe('splitArgs / joinArgs', () => {
  it('공백과 줄바꿈으로 나눈다', () => {
    expect(splitArgs('-Xmx1g  -Dfile.encoding=UTF-8\n--add-opens java.base/java.lang=ALL-UNNAMED')).toEqual([
      '-Xmx1g',
      '-Dfile.encoding=UTF-8',
      '--add-opens',
      'java.base/java.lang=ALL-UNNAMED',
    ])
  })

  it('따옴표로 묶인 부분은 하나로 본다', () => {
    expect(splitArgs(`--title="My App" '--x=a "b"' ""`)).toEqual(['--title=My App', '--x=a "b"', ''])
  })

  it('백슬래시는 그대로 둔다 (Windows 경로)', () => {
    expect(splitArgs('C:\\Program Files\\x "C:\\Program Files\\Java"')).toEqual(['C:\\Program', 'Files\\x', 'C:\\Program Files\\Java'])
  })

  it('왕복 변환이 보존된다', () => {
    const args = ['--a=1', 'two words', 'say "hi"', '']
    expect(splitArgs(joinArgs(args))).toEqual(args)
  })
})

describe('목록 파서', () => {
  it('쉼표 목록', () => {
    expect(splitList(' local, db ,,')).toEqual(['local', 'db'])
  })
  it('포트', () => {
    expect(parsePorts('8080, 5173 99999 abc 0')).toEqual([8080, 5173])
  })
})

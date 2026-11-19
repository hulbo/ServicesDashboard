// 서비스 편집 폼용 도우미: 편집하기 쉬운 형태로 채우고, 저장 전에 쓸모없는 값을 걷어 낸다.

import type { Service, ServiceKind } from './types'
import { JAVA_KINDS, NODE_KINDS } from './types'

const BLOCKS = ['npm', 'node', 'gradle', 'maven', 'jar', 'shell', 'spring'] as const

const BLOCKS_BY_KIND: Record<ServiceKind, readonly string[]> = {
  npm: ['npm'],
  node: ['node'],
  'gradle-boot': ['gradle', 'spring'],
  gradle: ['gradle'],
  'maven-boot': ['maven', 'spring'],
  maven: ['maven'],
  jar: ['jar', 'spring'],
  shell: ['shell'],
  'java-main': BLOCKS,
}

const JVM_ARG_KINDS: ServiceKind[] = ['gradle-boot', 'gradle', 'maven-boot', 'jar', 'java-main']
const ARG_KINDS: ServiceKind[] = ['npm', 'node', 'gradle-boot', 'gradle', 'maven-boot', 'jar', 'java-main']

export function usesJvmArgs(kind: ServiceKind) {
  return JVM_ARG_KINDS.includes(kind)
}

export function usesArgs(kind: ServiceKind) {
  return ARG_KINDS.includes(kind)
}

/** 폼에서 바로 바인딩할 수 있도록 하위 객체를 모두 만들어 둔다. */
export function normalizeForEdit(src: Service): Service {
  const s: Service = structuredClone(src)
  s.env ??= {}
  s.runtime ??= {}
  s.spring ??= {}
  s.npm ??= { script: '' }
  s.npm.packageManager ??= 'npm'
  s.node ??= { entry: '' }
  s.gradle ??= {}
  s.maven ??= {}
  s.jar ??= { path: '' }
  s.shell ??= { command: '' }
  s.shell.shell ??= 'cmd'
  s.encoding ??= 'auto'
  return s
}

function isPlainObject(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
}

function prune(obj: Record<string, unknown>) {
  for (const [k, v] of Object.entries(obj)) {
    if (v === undefined || v === null || v === '') delete obj[k]
    else if (Array.isArray(v)) {
      const arr = v.filter((x) => x !== '' && x !== null && x !== undefined)
      if (arr.length === 0) delete obj[k]
      else obj[k] = arr
    } else if (isPlainObject(v)) {
      prune(v)
      if (Object.keys(v).length === 0) delete obj[k]
    }
  }
}

/** 저장할 형태로 정리한다: 현재 타입과 관계없는 블록과 빈 값을 지운다. 알 수 없는 필드는 그대로 둔다. */
export function cleanForSave(src: Service): Service {
  const s = structuredClone(src) as Service & Record<string, unknown>
  const allowed = BLOCKS_BY_KIND[s.type] ?? BLOCKS
  for (const b of BLOCKS) if (!allowed.includes(b)) delete s[b]
  if (s.type !== 'gradle' && s.gradle) delete s.gradle.tasks
  if (s.type !== 'maven' && s.maven) delete s.maven.goals
  if (s.npm?.packageManager === 'npm') delete s.npm.packageManager
  if (s.shell?.shell === 'cmd') delete s.shell.shell
  if (!usesJvmArgs(s.type)) delete s.jvmArgs
  if (!usesArgs(s.type)) delete s.args
  if (s.runtime) {
    if (!JAVA_KINDS.includes(s.type)) delete s.runtime.jdk
    if (!NODE_KINDS.includes(s.type)) delete s.runtime.node
  }
  if (s.encoding === 'auto') delete s.encoding
  if (typeof s.name === 'string') s.name = s.name.trim() || '이름 없음'
  // id·name·type은 비어 있지 않으므로 prune에서 지워지지 않는다 (키 순서도 유지된다).
  prune(s)
  return s
}

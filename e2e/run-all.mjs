// 실제 앱 E2E를 차례로 돌린다.  node e2e/run-all.mjs [이름...]   (예: node e2e/run-all.mjs console fonts)
import { spawnSync } from 'node:child_process'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { EXIT_SKIP } from './lib.mjs'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const ALL = ['console', 'reorder', 'fonts', 'toolbar', 'autohide', 'update']
const picked = process.argv.slice(2)
const unknown = picked.filter((n) => !ALL.includes(n))
if (unknown.length) {
  console.error(`모르는 시험: ${unknown.join(', ')}\n있는 시험: ${ALL.join(', ')}`)
  process.exit(2)
}

const summary = []
for (const name of picked.length ? picked : ALL) {
  console.log(`\n━━ ${name} ━━`)
  const started = Date.now()
  const r = spawnSync(process.execPath, [path.join(HERE, `${name}.mjs`)], { stdio: 'inherit' })
  const secs = ((Date.now() - started) / 1000).toFixed(0)
  summary.push({ name, result: r.status === 0 ? '통과' : r.status === EXIT_SKIP ? '건너뜀' : '실패', secs })
}

console.log('\n━━ 요약 ━━')
for (const s of summary) console.log(`${s.result.padEnd(4)}  ${s.name}  (${s.secs}초)`)
process.exit(summary.some((s) => s.result === '실패') ? 1 : 0)

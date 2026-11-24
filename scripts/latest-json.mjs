// 앱 안 업데이트가 읽는 latest.json을 만든다. 릴리스에 설치본과 함께 올린다.
//
//   node scripts/latest-json.mjs <버전> <설치본.exe.sig> <노트 파일> [출력 파일=latest.json] [설치본 주소]
//
// - 버전: 0.1.11 (앞의 v는 빼도 되고 붙여도 된다)
// - .sig: CI 산출물(services-dashboard-windows)에 설치본과 함께 들어 있는 서명 파일
// - 노트 파일: 업데이트 안내에 보여 줄 짧은 글 (평문)
// - 설치본 주소: 생략하면 GitHub 릴리스의 ServicesDashboard_<버전>_x64-setup.exe
import fs from 'node:fs'

const [, , rawVersion, sigPath, notesPath, out = 'latest.json', urlArg] = process.argv
if (!rawVersion || !sigPath || !notesPath) {
  console.error('사용법: node scripts/latest-json.mjs <버전> <설치본.exe.sig> <노트 파일> [출력 파일] [설치본 주소]')
  process.exit(2)
}
const version = rawVersion.replace(/^v/, '')
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`버전 형식이 잘못됐습니다: ${rawVersion}`)
  process.exit(2)
}
const signature = fs.readFileSync(sigPath, 'utf8').trim()
if (!signature) {
  console.error(`서명 파일이 비어 있습니다: ${sigPath}`)
  process.exit(2)
}
const url = urlArg || `https://github.com/hulbo/ServicesDashboard/releases/download/v${version}/ServicesDashboard_${version}_x64-setup.exe`

const latest = {
  version,
  notes: fs.readFileSync(notesPath, 'utf8').trim(),
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
  platforms: {
    'windows-x86_64': { signature, url },
  },
}
fs.writeFileSync(out, JSON.stringify(latest, null, 2) + '\n')
console.log(`${out}: v${version} → ${url}`)

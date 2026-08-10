#!/usr/bin/env node
/**
 * 版本号一致性检查 — 确保四处版本号同步
 *
 * 检查:
 *   package.json        → version
 *   src-tauri/Cargo.toml → version
 *   src-tauri/tauri.conf.json → version
 *   src/core/constants.ts → APP_VERSION
 *
 * 用法: node scripts/check-version-sync.js
 * 返回: 0=一致, 1=不一致(阻塞 commit)
 */
const fs = require('fs')
const path = require('path')

const root = path.resolve(__dirname, '..')

// 从四个文件提取版本号
function getVersion(file, pattern) {
  const content = fs.readFileSync(path.join(root, file), 'utf8')
  const match = content.match(pattern)
  return match ? match[1] : null
}

const sources = [
  { file: 'package.json', version: getVersion('package.json', /"version":\s*"([^"]+)"/) },
  { file: 'src-tauri/Cargo.toml', version: getVersion('src-tauri/Cargo.toml', /^version\s*=\s*"([^"]+)"/m) },
  { file: 'src-tauri/tauri.conf.json', version: getVersion('src-tauri/tauri.conf.json', /"version":\s*"([^"]+)"/) },
  { file: 'src/core/constants.ts', version: getVersion('src/core/constants.ts', /APP_VERSION\s*=\s*'([^']+)'/) },
]

const versions = sources.map(s => s.version)
const unique = [...new Set(versions)]

if (unique.length === 1) {
  console.log(`✓ 版本号一致: ${unique[0]}`)
  process.exit(0)
}

console.error('❌ 版本号不一致!')
console.error('')
sources.forEach(s => {
  console.error(`  ${s.file}: ${s.version}`)
})
console.error('')
console.error('请统一四处版本号后再提交。')
process.exit(1)

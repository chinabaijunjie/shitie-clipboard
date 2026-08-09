/**
 * Icon 生成脚本 — 从 SVG 源文件生成全套应用图标
 *
 * 生成:
 *   - 1024x1024.png  (源高清图)
 *   - 512x512.png
 *   - 128x128.png
 *   - 128x128@2x.png (256x256)
 *   - 32x32.png
 *   - icon.icns      (macOS,通过 iconutil)
 *   - icon.ico       (Windows,多尺寸嵌入)
 *
 * 用法: node design-mockups/generate-icons.js
 */
const fs = require('fs')
const path = require('path')
const { Resvg } = require('@resvg/resvg-js')
const { execSync } = require('child_process')

const MOCKUP_DIR = path.join(__dirname)
const ICONS_DIR = path.join(__dirname, '..', 'src-tauri', 'icons')

// ====== SVG 源(全尺寸精版) ======
const svgFull = fs.readFileSync(path.join(MOCKUP_DIR, 'icon-source.svg'), 'utf-8')

// ====== SVG 源(小尺寸简化版 — 去掉文字线,加粗描边) ======
const svgSmall = `<svg viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="bg-s" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#1e3a5f"/><stop offset="50%" stop-color="#16213e"/><stop offset="100%" stop-color="#0a0a1a"/>
    </linearGradient>
    <linearGradient id="glass-s" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="rgba(90,200,250,0.22)"/><stop offset="100%" stop-color="rgba(90,200,250,0.08)"/>
    </linearGradient>
    <linearGradient id="clip-s" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#7fd8f5"/><stop offset="50%" stop-color="#3eb5e8"/><stop offset="100%" stop-color="#2980b9"/>
    </linearGradient>
    <linearGradient id="gold-s" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#f0d090"/><stop offset="40%" stop-color="#e8c87a"/><stop offset="70%" stop-color="#c9a04e"/><stop offset="100%" stop-color="#b08828"/>
    </linearGradient>
    <radialGradient id="glow-s" cx="50%" cy="20%" r="40%">
      <stop offset="0%" stop-color="rgba(90,200,250,0.3)"/><stop offset="100%" stop-color="rgba(90,200,250,0)"/>
    </radialGradient>
  </defs>
  <rect width="1024" height="1024" rx="224" fill="url(#bg-s)"/>
  <rect width="1024" height="1024" rx="224" fill="url(#glow-s)"/>
  <rect x="260" y="230" width="504" height="620" rx="40" fill="url(#glass-s)" stroke="url(#gold-s)" stroke-width="8"/>
  <rect x="404" y="174" width="216" height="96" rx="24" fill="url(#clip-s)" stroke="url(#gold-s)" stroke-width="8"/>
  <rect x="418" y="190" width="188" height="24" rx="12" fill="rgba(255,255,255,0.4)"/>
  <circle cx="512" cy="222" r="24" fill="#0a1a2e" opacity="0.4" stroke="url(#gold-s)" stroke-width="5"/>
  <circle cx="512" cy="222" r="11" fill="rgba(127,216,245,0.65)"/>
</svg>`

// ====== 渲染 SVG → PNG Buffer ======
function renderPng(svgString, size) {
  const resvg = new Resvg(svgString, {
    fitTo: { mode: 'width', value: size },
    background: 'transparent',
  })
  const pngData = resvg.render().asPng()
  return Buffer.from(pngData)
}

// ====== 确保 icons 目录存在 ======
if (!fs.existsSync(ICONS_DIR)) {
  fs.mkdirSync(ICONS_DIR, { recursive: true })
}

// ====== 备份旧图标(加时间戳) ======
const timestamp = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19)
const backupDir = path.join(ICONS_DIR, `backup-old-${timestamp}`)
if (!fs.existsSync(backupDir)) {
  fs.mkdirSync(backupDir)
  const oldFiles = ['32x32.png', '128x128.png', '128x128@2x.png', 'icon.icns', 'icon.ico']
  oldFiles.forEach(f => {
    const src = path.join(ICONS_DIR, f)
    if (fs.existsSync(src)) {
      fs.copyFileSync(src, path.join(backupDir, f))
    }
  })
  console.log(`✓ 旧图标已备份到 ${path.basename(backupDir)}/`)
}

// ====== 生成 PNG(精版用于 ≥128,简化版用于 ≤64) ======
const pngConfigs = [
  { file: '1024x1024.png', size: 1024, svg: svgFull },
  { file: '512x512.png', size: 512, svg: svgFull },
  { file: '128x128@2x.png', size: 256, svg: svgFull },
  { file: '128x128.png', size: 128, svg: svgFull },
  { file: '64x64.png', size: 64, svg: svgSmall },
  { file: '32x32.png', size: 32, svg: svgSmall },
  { file: '16x16.png', size: 16, svg: svgSmall },
]

console.log('\n--- 生成 PNG ---')
pngConfigs.forEach(({ file, size, svg }) => {
  const buf = renderPng(svg, size)
  fs.writeFileSync(path.join(ICONS_DIR, file), buf)
  console.log(`✓ ${file} (${size}x${size}, ${Math.round(buf.length / 1024)}KB)`)
})

// ====== 生成 .icns(macOS,通过 iconutil) ======
console.log('\n--- 生成 .icns ---')
const iconsetDir = path.join(ICONS_DIR, 'icon.iconset')
if (fs.existsSync(iconsetDir)) {
  fs.rmSync(iconsetDir, { recursive: true })
}
fs.mkdirSync(iconsetDir)

// macOS iconset 需要的标准尺寸
const iconsetSizes = [
  { name: 'icon_16x16.png', size: 16 },
  { name: 'icon_16x16@2x.png', size: 32 },
  { name: 'icon_32x32.png', size: 32 },
  { name: 'icon_32x32@2x.png', size: 64 },
  { name: 'icon_128x128.png', size: 128 },
  { name: 'icon_128x128@2x.png', size: 256 },
  { name: 'icon_256x256.png', size: 256 },
  { name: 'icon_256x256@2x.png', size: 512 },
  { name: 'icon_512x512.png', size: 512 },
  { name: 'icon_512x512@2x.png', size: 1024 },
]

iconsetSizes.forEach(({ name, size }) => {
  const svg = size <= 64 ? svgSmall : svgFull
  const buf = renderPng(svg, size)
  fs.writeFileSync(path.join(iconsetDir, name), buf)
})

try {
  execSync(`iconutil -c icns "${iconsetDir}" -o "${path.join(ICONS_DIR, 'icon.icns')}"`, { stdio: 'pipe' })
  console.log('✓ icon.icns (macOS)')
} catch (e) {
  console.error('✗ iconutil 失败:', e.message)
}

// 清理临时 iconset 目录
fs.rmSync(iconsetDir, { recursive: true })

// ====== 生成 .ico(Windows,多尺寸嵌入) ======
console.log('\n--- 生成 .ico ---')
// ICO 文件格式:6字节头 + 目录条目(16字节/个) + PNG 数据
// Windows 支持 PNG 格式的 ICO(Vista+)
const icoSizes = [16, 32, 48, 64, 128, 256]
const icoPngs = icoSizes.map(size => ({
  size,
  data: renderPng(size <= 64 ? svgSmall : svgFull, size),
}))

// ICO header(6 bytes)
const header = Buffer.alloc(6)
header.writeUInt16LE(0, 0)     // reserved
header.writeUInt16LE(1, 2)     // type = ICO
header.writeUInt16LE(icoPngs.length, 4) // count

// 目录条目(16 bytes each)
const dirSize = 16 * icoPngs.length
let dataOffset = 6 + dirSize
const dirEntries = []
icoPngs.forEach(({ size, data }) => {
  const entry = Buffer.alloc(16)
  entry.writeUInt8(size >= 256 ? 0 : size, 0)   // width (0 = 256)
  entry.writeUInt8(size >= 256 ? 0 : size, 1)   // height
  entry.writeUInt8(0, 2)                         // palette
  entry.writeUInt8(0, 3)                         // reserved
  entry.writeUInt16LE(1, 4)                      // color planes
  entry.writeUInt16LE(32, 6)                     // bits per pixel
  entry.writeUInt32LE(data.length, 8)            // image size
  entry.writeUInt32LE(dataOffset, 12)            // offset
  dirEntries.push(entry)
  dataOffset += data.length
})

const icoBuf = Buffer.concat([header, ...dirEntries, ...icoPngs.map(p => p.data)])
fs.writeFileSync(path.join(ICONS_DIR, 'icon.ico'), icoBuf)
console.log(`✓ icon.ico (Windows, ${icoSizes.join('/')}px, ${Math.round(icoBuf.length / 1024)}KB)`)

console.log('\n✅ 全套图标生成完毕!')
console.log(`   输出目录: ${ICONS_DIR}`)

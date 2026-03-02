/**
 * Assemble individual floor pattern PNGs into a spritesheet.
 *
 * Reads individual 16×16 floor patterns from raw-sprites/floors/ and
 * assembles them into a horizontal strip expected by the engine.
 *
 * Input structure:
 *   raw-sprites/floors/
 *     floor_0.png, floor_1.png, ... (each 16×16)
 *
 * Output: webview-ui/public/assets/floors.png (N*16 × 16)
 *
 * Run: npx tsx scripts/assemble-floors.ts
 */

import * as fs from 'fs'
import * as path from 'path'
import { PNG } from 'pngjs'

const TILE_SIZE = 16
const DEFAULT_PATTERN_COUNT = 7

const RAW_DIR = path.join(__dirname, '..', 'raw-sprites', 'floors')
const OUT_PATH = path.join(__dirname, '..', 'webview-ui', 'public', 'assets', 'floors.png')

function loadPng(filePath: string): PNG {
  const buffer = fs.readFileSync(filePath)
  return PNG.sync.read(buffer)
}

function blitFrame(dest: PNG, src: PNG, destX: number, destY: number): void {
  for (let y = 0; y < src.height; y++) {
    for (let x = 0; x < src.width; x++) {
      const srcIdx = (y * src.width + x) * 4
      const dstIdx = ((destY + y) * dest.width + (destX + x)) * 4
      dest.data[dstIdx] = src.data[srcIdx]
      dest.data[dstIdx + 1] = src.data[srcIdx + 1]
      dest.data[dstIdx + 2] = src.data[srcIdx + 2]
      dest.data[dstIdx + 3] = src.data[srcIdx + 3]
    }
  }
}

// Main
if (!fs.existsSync(RAW_DIR)) {
  console.error(`Input directory not found: ${RAW_DIR}`)
  console.log('\nExpected structure:')
  console.log('  raw-sprites/floors/')
  console.log('    floor_0.png, floor_1.png, ... (each 16×16)')
  process.exit(1)
}

const files = fs.readdirSync(RAW_DIR)
  .filter(f => f.match(/^floor_\d+\.png$/))
  .sort((a, b) => {
    const numA = parseInt(a.match(/\d+/)![0])
    const numB = parseInt(b.match(/\d+/)![0])
    return numA - numB
  })

if (files.length === 0) {
  console.error(`No floor_N.png files found in ${RAW_DIR}`)
  process.exit(1)
}

console.log(`Assembling ${files.length} floor pattern(s)...\n`)

const sheetW = TILE_SIZE * files.length
const sheetH = TILE_SIZE
const png = new PNG({ width: sheetW, height: sheetH })

for (let i = 0; i < files.length; i++) {
  const filePath = path.join(RAW_DIR, files[i])
  const tile = loadPng(filePath)

  if (tile.width !== TILE_SIZE || tile.height !== TILE_SIZE) {
    console.error(`ERROR: ${files[i]} is ${tile.width}×${tile.height}, expected ${TILE_SIZE}×${TILE_SIZE}`)
    process.exit(1)
  }

  blitFrame(png, tile, i * TILE_SIZE, 0)
  console.log(`  ${files[i]} → slot ${i}`)
}

fs.mkdirSync(path.dirname(OUT_PATH), { recursive: true })
fs.writeFileSync(OUT_PATH, PNG.sync.write(png))
console.log(`\n✓ ${OUT_PATH} (${sheetW}×${sheetH})`)

if (files.length !== DEFAULT_PATTERN_COUNT) {
  console.warn(`\n⚠ Pattern count is ${files.length} (default: ${DEFAULT_PATTERN_COUNT})`)
  console.warn('  Update FLOOR_PATTERN_COUNT in webview-ui/src/assets/assetLoader.ts if changed')
}

console.log('\nDone!')

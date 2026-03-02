/**
 * Assemble individual wall tile PNGs into a spritesheet.
 *
 * Reads 16 wall tile PNGs (one per 4-bit auto-tile bitmask) from
 * raw-sprites/walls/ and assembles them into the 64×128 grid
 * expected by the engine.
 *
 * Input structure:
 *   raw-sprites/walls/
 *     wall_0.png  (bitmask 0000 = no connections)
 *     wall_1.png  (bitmask 0001 = North)
 *     wall_2.png  (bitmask 0010 = East)
 *     wall_3.png  (bitmask 0011 = North+East)
 *     ...
 *     wall_15.png (bitmask 1111 = all connections)
 *
 *   Each tile: 16×32px (16px plan view + 16px 3D face above)
 *
 * Bitmask encoding:
 *   bit 0 (1) = North, bit 1 (2) = East,
 *   bit 2 (4) = South, bit 3 (8) = West
 *
 * Output: webview-ui/public/assets/walls.png (64×128)
 *   4 columns × 4 rows, each cell 16×32
 *
 * Run: npx tsx scripts/assemble-walls.ts
 */

import * as fs from 'fs'
import * as path from 'path'
import { PNG } from 'pngjs'

const PIECE_W = 16
const PIECE_H = 32
const GRID_COLS = 4
const GRID_ROWS = 4
const BITMASK_COUNT = 16
const SHEET_W = PIECE_W * GRID_COLS   // 64
const SHEET_H = PIECE_H * GRID_ROWS   // 128

const RAW_DIR = path.join(__dirname, '..', 'raw-sprites', 'walls')
const OUT_PATH = path.join(__dirname, '..', 'webview-ui', 'public', 'assets', 'walls.png')

const BITMASK_LABELS = [
  'none', 'N', 'E', 'N+E',
  'S', 'N+S', 'E+S', 'N+E+S',
  'W', 'N+W', 'E+W', 'N+E+W',
  'S+W', 'N+S+W', 'E+S+W', 'N+E+S+W',
]

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
  console.log('  raw-sprites/walls/')
  console.log('    wall_0.png through wall_15.png (each 16×32)')
  console.log('\n  Bitmask encoding:')
  for (let i = 0; i < BITMASK_COUNT; i++) {
    console.log(`    wall_${i}.png  = ${BITMASK_LABELS[i]} (${i.toString(2).padStart(4, '0')})`)
  }
  process.exit(1)
}

console.log(`Assembling ${BITMASK_COUNT} wall tiles...\n`)

const png = new PNG({ width: SHEET_W, height: SHEET_H })
let loadedCount = 0
const missing: string[] = []

for (let mask = 0; mask < BITMASK_COUNT; mask++) {
  const filename = `wall_${mask}.png`
  const filePath = path.join(RAW_DIR, filename)

  if (!fs.existsSync(filePath)) {
    missing.push(`  wall_${mask}.png (${BITMASK_LABELS[mask]})`)
    continue
  }

  const tile = loadPng(filePath)

  if (tile.width !== PIECE_W || tile.height !== PIECE_H) {
    console.error(`ERROR: ${filename} is ${tile.width}×${tile.height}, expected ${PIECE_W}×${PIECE_H}`)
    process.exit(1)
  }

  const col = mask % GRID_COLS
  const row = Math.floor(mask / GRID_COLS)
  blitFrame(png, tile, col * PIECE_W, row * PIECE_H)
  console.log(`  wall_${mask}.png (${BITMASK_LABELS[mask]}) → grid [${col}, ${row}]`)
  loadedCount++
}

if (missing.length > 0) {
  console.warn(`\n⚠ ${missing.length} missing tile(s):`)
  missing.forEach(m => console.warn(m))
}

fs.mkdirSync(path.dirname(OUT_PATH), { recursive: true })
fs.writeFileSync(OUT_PATH, PNG.sync.write(png))
console.log(`\n✓ ${OUT_PATH} (${SHEET_W}×${SHEET_H})`)
console.log(`  ${loadedCount}/${BITMASK_COUNT} tiles assembled`)
console.log('\nDone!')

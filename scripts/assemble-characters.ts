/**
 * Assemble individual character frame PNGs into spritesheets.
 *
 * Reads individual frame PNGs from raw-sprites/characters/char_N/ and
 * assembles them into the 112×96 spritesheet format expected by the engine.
 *
 * Input structure:
 *   raw-sprites/characters/char_0/
 *     down_walk1.png, down_walk2.png, down_walk3.png,
 *     down_type1.png, down_type2.png, down_read1.png, down_read2.png,
 *     up_walk1.png, ..., right_read2.png  (21 frames per character)
 *
 * Output: webview-ui/public/assets/characters/char_N.png (112×96)
 *
 * Run: npx tsx scripts/assemble-characters.ts
 */

import * as fs from 'fs'
import * as path from 'path'
import { PNG } from 'pngjs'

const FRAME_W = 16
const FRAME_H = 32
const DIRECTIONS = ['down', 'up', 'right'] as const
const FRAMES = ['walk1', 'walk2', 'walk3', 'type1', 'type2', 'read1', 'read2'] as const
const SHEET_W = FRAME_W * FRAMES.length   // 112
const SHEET_H = FRAME_H * DIRECTIONS.length // 96

const RAW_DIR = path.join(__dirname, '..', 'raw-sprites', 'characters')
const OUT_DIR = path.join(__dirname, '..', 'webview-ui', 'public', 'assets', 'characters')

function loadPng(filePath: string): PNG {
  const buffer = fs.readFileSync(filePath)
  return PNG.sync.read(buffer)
}

function blitFrame(dest: PNG, src: PNG, destX: number, destY: number): void {
  const srcW = src.width
  const srcH = src.height
  for (let y = 0; y < srcH; y++) {
    for (let x = 0; x < srcW; x++) {
      const srcIdx = (y * srcW + x) * 4
      const dstIdx = ((destY + y) * dest.width + (destX + x)) * 4
      dest.data[dstIdx] = src.data[srcIdx]
      dest.data[dstIdx + 1] = src.data[srcIdx + 1]
      dest.data[dstIdx + 2] = src.data[srcIdx + 2]
      dest.data[dstIdx + 3] = src.data[srcIdx + 3]
    }
  }
}

function assembleCharacter(charDir: string, charName: string): Buffer {
  const png = new PNG({ width: SHEET_W, height: SHEET_H })

  let frameCount = 0
  const warnings: string[] = []

  for (let dirIdx = 0; dirIdx < DIRECTIONS.length; dirIdx++) {
    const dir = DIRECTIONS[dirIdx]
    for (let frameIdx = 0; frameIdx < FRAMES.length; frameIdx++) {
      const frameName = FRAMES[frameIdx]
      const filename = `${dir}_${frameName}.png`
      const framePath = path.join(charDir, filename)

      if (!fs.existsSync(framePath)) {
        warnings.push(`  Missing: ${filename}`)
        continue
      }

      const frame = loadPng(framePath)

      if (frame.width !== FRAME_W || frame.height !== FRAME_H) {
        console.error(`  ERROR: ${filename} is ${frame.width}×${frame.height}, expected ${FRAME_W}×${FRAME_H}`)
        process.exit(1)
      }

      const destX = frameIdx * FRAME_W
      const destY = dirIdx * FRAME_H
      blitFrame(png, frame, destX, destY)
      frameCount++
    }
  }

  if (warnings.length > 0) {
    console.warn(`⚠ ${charName}: ${warnings.length} missing frame(s):`)
    warnings.forEach(w => console.warn(w))
  }

  console.log(`  ${charName}: ${frameCount}/${DIRECTIONS.length * FRAMES.length} frames assembled`)

  return PNG.sync.write(png)
}

// Main
if (!fs.existsSync(RAW_DIR)) {
  console.error(`Input directory not found: ${RAW_DIR}`)
  console.log('\nExpected structure:')
  console.log('  raw-sprites/characters/char_0/')
  console.log('    down_walk1.png, down_walk2.png, down_walk3.png,')
  console.log('    down_type1.png, down_type2.png, down_read1.png, down_read2.png,')
  console.log('    up_walk1.png, ..., up_read2.png,')
  console.log('    right_walk1.png, ..., right_read2.png')
  console.log(`\n  Each frame: ${FRAME_W}×${FRAME_H}px PNG`)
  process.exit(1)
}

const charDirs = fs.readdirSync(RAW_DIR)
  .filter(name => fs.statSync(path.join(RAW_DIR, name)).isDirectory())
  .sort()

if (charDirs.length === 0) {
  console.error(`No character folders found in ${RAW_DIR}`)
  process.exit(1)
}

fs.mkdirSync(OUT_DIR, { recursive: true })

console.log(`Assembling ${charDirs.length} character(s)...\n`)

for (const charName of charDirs) {
  const charDir = path.join(RAW_DIR, charName)
  const buffer = assembleCharacter(charDir, charName)
  const outPath = path.join(OUT_DIR, `${charName}.png`)
  fs.writeFileSync(outPath, buffer)
  console.log(`✓ ${outPath} (${SHEET_W}×${SHEET_H})\n`)
}

console.log('Done!')
console.log(`\nSpritesheet layout: ${SHEET_W}×${SHEET_H}`)
console.log('  7 frames × 16px wide, 3 rows × 32px tall')
console.log('  Row 0: down, Row 1: up, Row 2: right')
console.log('  Frame order: walk1, walk2, walk3, type1, type2, read1, read2')

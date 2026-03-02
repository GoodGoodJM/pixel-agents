/**
 * Assemble all sprite assets from raw-sprites/ into engine-ready spritesheets.
 *
 * Runs all assembly scripts in sequence:
 *   1. Characters: raw-sprites/characters/char_N/ → assets/characters/char_N.png
 *   2. Floors: raw-sprites/floors/floor_N.png → assets/floors.png
 *   3. Walls: raw-sprites/walls/wall_N.png → assets/walls.png
 *
 * Run: npx tsx scripts/assemble-sprites.ts
 */

import * as fs from 'fs'
import * as path from 'path'
import { execSync } from 'child_process'

const SCRIPTS_DIR = __dirname
const RAW_DIR = path.join(__dirname, '..', 'raw-sprites')

const tasks = [
  { name: 'Characters', script: 'assemble-characters.ts', inputDir: 'characters' },
  { name: 'Floors', script: 'assemble-floors.ts', inputDir: 'floors' },
  { name: 'Walls', script: 'assemble-walls.ts', inputDir: 'walls' },
]

console.log('=== Sprite Assembly ===\n')

let ran = 0

for (const task of tasks) {
  const inputDir = path.join(RAW_DIR, task.inputDir)
  if (!fs.existsSync(inputDir)) {
    console.log(`⏭ ${task.name}: skipped (${path.relative(process.cwd(), inputDir)}/ not found)\n`)
    continue
  }

  console.log(`── ${task.name} ──`)
  const scriptPath = path.join(SCRIPTS_DIR, task.script)
  try {
    execSync(`npx tsx "${scriptPath}"`, { stdio: 'inherit', cwd: path.join(__dirname, '..') })
    ran++
  } catch {
    console.error(`\n✗ ${task.name} failed`)
    process.exit(1)
  }
  console.log('')
}

if (ran === 0) {
  console.log('Nothing to assemble. Create input files in raw-sprites/:')
  console.log('')
  console.log('  raw-sprites/')
  console.log('  ├── characters/char_0/     ← 21 frames (down/up/right × 7 animations)')
  console.log('  │     down_walk1.png, down_walk2.png, down_walk3.png,')
  console.log('  │     down_type1.png, down_type2.png, down_read1.png, down_read2.png,')
  console.log('  │     up_walk1.png, ..., right_read2.png')
  console.log('  │     (each 16×32px)')
  console.log('  ├── floors/                ← floor_0.png ~ floor_6.png (each 16×16px)')
  console.log('  └── walls/                 ← wall_0.png ~ wall_15.png (each 16×32px)')
} else {
  console.log(`=== Done! ${ran} asset type(s) assembled ===`)
}

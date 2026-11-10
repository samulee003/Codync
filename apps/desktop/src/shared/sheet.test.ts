import assert from 'node:assert/strict'
import { test } from 'node:test'
import { chartPoints, classInfo, levelProgress } from './sheet.ts'

test('level progress is the share of the level earned', () => {
  assert.equal(levelProgress({ xp: 20, levelXp: 20, nextLevelXp: 80 }), 0)
  assert.equal(levelProgress({ xp: 50, levelXp: 20, nextLevelXp: 80 }), 50)
  assert.equal(levelProgress({ xp: 500, levelXp: 20, nextLevelXp: 80 }), 100)
  assert.equal(levelProgress({ xp: 0, levelXp: 0, nextLevelXp: 0 }), 0, 'a host that sent no span')
})

test('a chart starts at the top and goes clockwise', () => {
  assert.equal(chartPoints([100, 100, 100, 100], 50, 40), '50.0,10.0 90.0,50.0 50.0,90.0 10.0,50.0')
  assert.equal(chartPoints([0, 50, 250, -5], 50, 40), '50.0,50.0 70.0,50.0 50.0,90.0 50.0,50.0', 'values stay within 0–100')
})

test('a bot without a class shows as none', () => {
  assert.equal(classInfo('scout').title, 'Scout')
  assert.equal(classInfo(null).title, 'No class')
})

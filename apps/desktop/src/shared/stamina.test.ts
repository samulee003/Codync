import assert from 'node:assert/strict'
import { test } from 'node:test'
import { staminaLevel } from './host-models.ts'

test('the bar turns yellow under half and red under a fifth', () => {
  assert.equal(staminaLevel(100), 'full')
  assert.equal(staminaLevel(50), 'full')
  assert.equal(staminaLevel(49), 'low')
  assert.equal(staminaLevel(20), 'low')
  assert.equal(staminaLevel(19), 'out')
  assert.equal(staminaLevel(0), 'out')
})

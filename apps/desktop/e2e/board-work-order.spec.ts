import { test, expect } from '@playwright/test'
import { attach } from './sandbox.ts'

// Real bots on the sandbox (a lead and a checker that take board work): the lead proposes a plan for a task with
// acceptance criteria, the user approves it on the board, and the checker marks every criterion. The task writes in
// /tmp/codync-work-order; screenshots land in test-results/.
test('a work order: plan to approve, then criteria checked one by one', async () => {
  test.setTimeout(420_000)
  const { browser, page } = await attach()
  const title = `days_in_month ${Date.now()}`
  try {
    await page.evaluate(() => localStorage.setItem('appLanguage', JSON.stringify('zh-Hant')))
    await page.reload()
    await page.locator('[title="團隊任務板"]').first().click()
    await page.locator('.board [title="新增任務"]').first().click()
    const form = page.locator('.board-form')
    await form.locator('input.board-input').first().fill(`寫一個 Python 函數 ${title}`)
    await form.locator('textarea.board-input').nth(0).fill('寫在 /tmp/codync-work-order/days.py，附 test_days.py。')
    await form.locator('textarea.board-input').nth(1).fill('- 2024 年 2 月有 29 日，2023 年 2 月有 28 日\n- python3 test_days.py 全部通過')
    await expect(form.locator('.switch[aria-checked="true"]')).toBeVisible()
    await page.screenshot({ path: 'test-results/1-new-task-form.png' })
    await form.getByRole('button', { name: '新增任務' }).click()

    const card = page.locator('.board-task', { hasText: title })
    await expect(card.getByRole('button', { name: '批准計劃' })).toBeVisible({ timeout: 180_000 })
    await expect(page.getByText(/待批准的計劃/)).toBeVisible()
    await page.screenshot({ path: 'test-results/2-plan-to-approve.png' })
    await card.getByRole('button', { name: '批准計劃' }).click()

    await expect(page.locator('.details-section', { hasText: /^完成/ }).locator('.board-task', { hasText: title }))
      .toBeVisible({ timeout: 240_000 })
    if (!(await card.getByText('驗收標準').isVisible())) await card.locator('.board-task-main').click()
    await expect(card.getByText('驗收標準')).toBeVisible()
    await expect(card.getByText('已批准的計劃')).toBeVisible()
    await card.scrollIntoViewIfNeeded()
    await page.screenshot({ path: 'test-results/3-checked.png' })
  } finally {
    await page.evaluate(() => localStorage.removeItem('appLanguage'))
    await browser.close()
  }
})

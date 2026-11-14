import type { BotClass, BotSheet } from '@shared/sheet'
import type { HostClient } from '../client/host-client'

/** How long after a board change the sheets are read again: one read for a burst of changes. */
const REFRESH_MS = 800

/**
 * One computer's character sheets (`host/src/board/sheet.rs`), by bot. The host counts what bots do on the board, so
 * the sheets are read again after board changes. A host without sheets leaves this empty.
 */
export class Sheets {
  readonly byBot = new Map<string, BotSheet>()
  /** Whether this computer's host has sheets (known once they were read). */
  available = false
  private timer: ReturnType<typeof setTimeout> | null = null
  private changed: () => void
  constructor(changed: () => void) { this.changed = changed }

  async load(client: HostClient | null) {
    if (!client) return
    let sheets: BotSheet[] | undefined
    try {
      sheets = (await client.botSheets()).sheets
    } catch {
      // An older host has no sheets: nothing to show.
      return
    }
    if (!Array.isArray(sheets)) return
    this.available = true
    this.byBot.clear()
    for (const sheet of sheets) this.byBot.set(sheet.botId, sheet)
    this.changed()
  }

  /** Reads the sheets again soon: the board changed, so a bot may have earned something. */
  refreshSoon(client: HostClient | null) {
    if (this.timer) return
    this.timer = setTimeout(() => {
      this.timer = null
      void this.load(client)
    }, REFRESH_MS)
  }

  async setClass(client: HostClient | null, botId: string, botClass: BotClass | null) {
    if (!client) throw new Error('Connect to this computer to set a class')
    const { sheet } = await client.setBotClass(botId, botClass)
    this.byBot.set(sheet.botId, sheet)
    this.changed()
  }
}

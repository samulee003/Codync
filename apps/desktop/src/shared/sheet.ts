// A bot's character sheet on the wire (`host/src/board/sheet.rs`): the class the user gives it, and a level and five
// abilities that grow with what it does on the team board.

export type BotClass = 'strategist' | 'warrior' | 'scholar' | 'scout' | 'inspector'

export type Ability = 'might' | 'intellect' | 'command' | 'accuracy' | 'insight'

export interface Deeds {
  finished: number
  firstTry: number
  sentBack: number
  checked: number
  caught: number
  planned: number
  notes: number
}

export interface BotSheet {
  botId: string
  class: BotClass | null
  level: number
  xp: number
  /** The experience at which this level started, and at which the next one does. */
  levelXp: number
  nextLevelXp: number
  /** Each 0–100. */
  abilities: Record<Ability, number>
  deeds: Deeds
}

/** The classes, as a picker lists them: name, emblem and what the class is for. */
export const BOT_CLASSES: { id: BotClass; title: string; symbol: string; role: string }[] = [
  { id: 'strategist', title: 'Strategist', symbol: 'brain', role: 'Plans and splits work' },
  { id: 'warrior', title: 'Warrior', symbol: 'hammer', role: 'Builds code and systems' },
  { id: 'scholar', title: 'Scholar', symbol: 'book', role: 'Writes documents and material' },
  { id: 'scout', title: 'Scout', symbol: 'magnifyingglass', role: 'Researches' },
  { id: 'inspector', title: 'Inspector', symbol: 'checkmark.seal', role: 'Checks finished work' },
]

/** A bot with no class yet. */
export const NO_CLASS = { title: 'No class', symbol: 'person' }

export function classInfo(id: BotClass | null | undefined): { title: string; symbol: string } {
  return BOT_CLASSES.find((c) => c.id === id) ?? NO_CLASS
}

/** The abilities in the order a sheet shows them, clockwise from the top of its chart. */
export const ABILITIES: { id: Ability; title: string }[] = [
  { id: 'might', title: 'Might' },
  { id: 'intellect', title: 'Intellect' },
  { id: 'command', title: 'Leadership' },
  { id: 'accuracy', title: 'Accuracy' },
  { id: 'insight', title: 'Insight' },
]

/** How far through its level a bot is, 0–100. */
export function levelProgress(sheet: Pick<BotSheet, 'xp' | 'levelXp' | 'nextLevelXp'>): number {
  const span = sheet.nextLevelXp - sheet.levelXp
  if (span <= 0) return 0
  return Math.max(0, Math.min(100, Math.round(((sheet.xp - sheet.levelXp) / span) * 100)))
}

/**
 * The corners of an ability chart: one point per value (0–100) around `center`, the first straight up, going
 * clockwise, at `radius` for 100. For an SVG `points` attribute.
 */
export function chartPoints(values: number[], center: number, radius: number): string {
  return values
    .map((value, i) => {
      const angle = (2 * Math.PI * i) / values.length - Math.PI / 2
      const reach = (radius * Math.max(0, Math.min(100, value))) / 100
      return `${(center + reach * Math.cos(angle)).toFixed(1)},${(center + reach * Math.sin(angle)).toFixed(1)}`
    })
    .join(' ')
}

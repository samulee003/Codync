// A bot's character sheet on the wire (`host/src/board/sheet.rs`): the class the user gives it, and a level and five
// abilities that grow with what it does on the team board.

export type BotClass = 'strategist' | 'ruler' | 'general' | 'infantry' | 'martialArtist'

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
  /** The class's rank: 1, then one more at each promotion (levels 5 and 10). */
  rank: number
  /** The level of the next promotion; null once fully promoted. */
  nextPromotion: number | null
  xp: number
  /** The experience at which this level started, and at which the next one does. */
  levelXp: number
  nextLevelXp: number
  /** Each 0–100. */
  abilities: Record<Ability, number>
  deeds: Deeds
}

/**
 * The classes, as a picker lists them: emblem, what the class is for, and its title at each rank (the first is the
 * class's name; the host promotes it at levels 5 and 10).
 */
export const BOT_CLASSES: { id: BotClass; titles: [string, string, string]; symbol: string; role: string }[] = [
  { id: 'strategist', titles: ['Strategist', 'Chief Strategist', 'Grand Strategist'], symbol: 'brain', role: 'Plans and splits work' },
  { id: 'ruler', titles: ['Ruler', 'Overlord', 'Emperor'], symbol: 'star', role: 'Leads, coordinates and assembles' },
  { id: 'general', titles: ['Commander', 'High Commander', 'Grand Commander'], symbol: 'bolt', role: 'Builds code and systems' },
  { id: 'infantry', titles: ['Infantry', 'Elite Infantry', 'Imperial Guard'], symbol: 'person.2', role: 'Steady, well-defined tasks' },
  { id: 'martialArtist', titles: ['Martial Artist', 'Martial Master', 'Martial Saint'], symbol: 'hand.raised', role: 'Checks finished work' },
]

/** A bot with no class yet. */
export const NO_CLASS = { title: 'No class', symbol: 'person' }

/** A class's name (its first title). */
export function className(id: BotClass): string {
  return BOT_CLASSES.find((c) => c.id === id)?.titles[0] ?? NO_CLASS.title
}

/** What a sheet shows for the bot: its class's title at its rank, and the class's emblem. */
export function classInfo(id: BotClass | null | undefined, rank = 1): { title: string; symbol: string } {
  const found = BOT_CLASSES.find((c) => c.id === id)
  if (!found) return NO_CLASS
  return { title: found.titles[Math.max(0, Math.min(found.titles.length - 1, rank - 1))]!, symbol: found.symbol }
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

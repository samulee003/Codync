import type { ArtifactCard } from './artifacts'
import type { CallLine } from './calls'
import type { FallbackReason } from './models'

export interface FileDiff {
  path: string
  added: number
  removed: number
  isNew: boolean
  startLine: number
  patch: string
}

export interface PermissionOption {
  optionId: string
  name: string
  /** allow_once | allow_always | reject_once | reject_always */
  kind: string
}

export interface PlanItem {
  content: string
  status: string
  priority?: string
}

/** One bot's emoji on a message. */
export interface BotReaction {
  bot: string
  emoji: string
}

/** The bots' reactions on one message, by emoji in the order each first appeared. */
export function groupBotReactions(reactions: BotReaction[]): { emoji: string; bots: string[] }[] {
  const groups: { emoji: string; bots: string[] }[] = []
  for (const r of reactions) {
    const group = groups.find((g) => g.emoji === r.emoji)
    if (group) group.bots.push(r.bot)
    else groups.push({ emoji: r.emoji, bots: [r.bot] })
  }
  return groups
}

/** The message a user message replies to, as the host quoted it. */
export interface ReplyQuote {
  id: string
  /** `user`, or the bot that wrote it. */
  author: string
  /** Its text (or file names), at most 500 characters. */
  text: string
  /** The artifact, when it quotes an artifact card: the message is about that artifact. */
  artifact?: string
}

export interface Attachment {
  id: string
  name: string
  size: number
}

export interface SharedFile extends Attachment {
  sha256: string
  /** A sound the chat plays in place, as the host stamps it; missing (an older host) means a plain file card. */
  playsInChat?: boolean
}

export const isImageName = (name: string) =>
  ['png', 'jpg', 'jpeg', 'heic', 'gif', 'webp', 'tiff', 'bmp'].includes(name.split('.').pop()?.toLowerCase() ?? '')

export const isAudioName = (name: string) => ['mp3', 'm4a', 'wav', 'aac'].includes(name.split('.').pop()?.toLowerCase() ?? '')

/** The largest shared image a chat fetches whole to show. */
const INLINE_BYTES = 20 * 1024 * 1024

/** A shared image small enough to show in the chat. */
export const showsAsImage = (file: SharedFile) => isImageName(file.name) && file.size <= INLINE_BYTES

/** The voice note playing at the top of a message's bubble; none when its files show as their own cards. */
export const voiceNoteOf = (data: EntryData) => {
  const file = data.voiceNote ? data.files?.[0] : undefined
  return file?.playsInChat ? file : undefined
}

export interface ConnectionRequest {
  [key: string]: unknown
}

export interface BotMessage {
  sourceBotId: string
  targetBotId: string
  text: string
  reply?: string
  detail?: string
  /** The peer's name when it is on another of the user's computers (its id is `name@computer`). */
  peerName?: string
}

/** "Earlier context was summarized": the harness's summary, when it gave one. */
export interface Compaction {
  summary?: string | null
  /** The host started it ahead of time; otherwise the harness did when its context filled. */
  byHost?: boolean
}

/** What a new conversation carries when the bot's own chat starts over; editable until the next message sends it. */
export interface HandoffCard {
  /** draft · sent · kept */
  status: string
  /** cold · away · full */
  reason?: string
  text?: string
}

/** notice: another agent of the bot's chain carried on, or (`back`) the bot's own is back. */
export interface AgentSwitch {
  from: string
  to: string
  back: boolean
  reason: FallbackReason | null
  resetsAt: number | null
  /** What the new agent was told, unfolded under the line. */
  carried: string | null
}

/** A choice card's (`request_decision`) lifecycle, as the host writes it in `status`. */
export type DecisionStatus = 'pending' | 'answered' | 'expired'

export interface EntryData {
  /** Structured conversation data added to the existing team-tool notice. */
  botMessage?: BotMessage
  /** notice: the agent summarized its context; kept out of the chat. */
  compaction?: Compaction
  /** notice: the bot's chat starts over with this handoff (see `HandoffNotice`). */
  handoff?: HandoffCard
  agentSwitch?: AgentSwitch
  /** notice (error): the bot has no fallbacks and its agent ran out; offers to add one. */
  addFallback?: boolean
  heading?: string
  delegationId?: string
  sourceBotId?: string
  targetBotId?: string
  connectionRequest?: ConnectionRequest
  /** A memory line an older host put in the chat. Memory lives in the bot's memory settings, so it isn't shown. */
  memory?: unknown
  memoryFile?: unknown
  routineId?: string
  runId?: string
  /** notice: a failed routine run the user can run again (`retryRoutineRun`). */
  retry?: boolean
  text?: string
  final?: boolean
  /** user: queued | sent | cancelled | failed · tool: pending | in_progress | completed | failed · permission: pending | answered | cancelled | expired */
  status?: string
  /** user: taken back from the queue before it went out (`unqueue`); no view reads it (`withoutRemoved`). */
  removed?: boolean
  clientNonce?: string
  title?: string
  toolKind?: string
  /** tool: whether it's the bot's own bookkeeping (composing its message, keeping its memory), left out of the
   * working steps; a host older than 2.25.0 leaves it out, and the steps then leave out the call too. */
  bookkeeping?: boolean
  output?: string
  diffs?: FileDiff[]
  locations?: { path: string; line?: number }[]
  options?: PermissionOption[]
  selected?: string
  /** An additive notice card; older clients still display its text. Its `status` is a `DecisionStatus`. */
  decision?: boolean
  /** What the bot asks the user to choose (`request_decision`). */
  question?: string
  /** decision: the choices, in the order the bot offered them. */
  choices?: string[]
  /** decision: which choice the bot suggests (`choices`' index). */
  recommended?: number | null
  command?: string
  detail?: string
  cwd?: string
  entries?: PlanItem[]
  /** notice: info | error */
  style?: string
  author?: string
  /** user: the message this one replies to. */
  replyTo?: ReplyQuote
  reactions?: string[]
  /** The bots' reactions, one per bot, oldest first. */
  botReactions?: BotReaction[]
  attachments?: Attachment[]
  files?: SharedFile[]
  /** agent: `files` holds the voice note of this message, which plays at the top of its bubble. */
  voiceNote?: boolean
  /** agent: an artifact the bot published (or updated) in this turn. */
  artifact?: ArtifactCard
  callSeconds?: number
  /** The voice call it was said in (user), answered during (agent), or that it ends (notice): the chat shows those
   * messages only in the call's conversation. */
  call?: string
  /** user: `voice` when a call's voice wrote it for the user (`send_to_bot`). */
  from?: string
  /** notice: what was said on a realtime call, oldest first. */
  transcript?: CallLine[]
}

export interface Entry {
  id: string
  seq: number
  botId: string
  rev: number
  /** user | agent | thought | tool | plan | permission | notice */
  kind: string
  turn: number
  data: EntryData
  createdAt: number
  updatedAt: number
}

const shownOf = new WeakMap<Entry[], Entry[]>()

/**
 * `list` as the views read it, without the messages taken back (`unqueue`): the store keeps those for its rev guard
 * against a late response. One array per list, because the chat's memos depend on its identity.
 */
export function withoutRemoved(list: Entry[]): Entry[] {
  let shown = shownOf.get(list)
  if (!shown) {
    shown = list.filter((e) => !e.data.removed)
    shownOf.set(list, shown)
  }
  return shown
}

/** Chat-visible (Grok Bot model): user messages, final replies, approval cards, notices. */
export function isChat(e: Entry): boolean {
  switch (e.kind) {
    case 'user':
    case 'permission':
      return true
    case 'notice':
      // Only a fresh conversation gets a line: memory lives in the bot's memory settings, a summary keeps the same
      // conversation going, and a kept handoff changed nothing.
      return !e.data.memory && !e.data.memoryFile && !e.data.compaction && e.data.handoff?.status !== 'kept'
    case 'agent':
      return e.data.final === true
    default:
      return false
  }
}

/** Messages a reply can quote: the user's and the bots' final replies, once the computer has them. */
export function isReplyable(e: Entry): boolean {
  return !e.id.startsWith('local-') && (e.kind === 'user' || (e.kind === 'agent' && e.data.final === true))
}

/** The quote a sending reply shows until the computer's own replaces it. */
export function quoteOf(e: Entry): ReplyQuote {
  const names = [...(e.data.attachments ?? []), ...(e.data.files ?? [])].map((f) => f.name).join(', ')
  const quote: ReplyQuote = { id: e.id, author: e.kind === 'user' ? 'user' : (e.data.author ?? e.botId), text: (e.data.text || names).slice(0, 500) }
  if (e.data.artifact) quote.artifact = e.data.artifact.id
  return quote
}

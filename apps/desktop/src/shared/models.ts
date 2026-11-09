import { normalizeDevServer, type DevServer } from './dev-servers.ts'

export * from './entry-models.ts'
export * from './host-models.ts'

// Wire types for codync-host (see host/src/api/). Field names match the
// host's camelCase JSON. Timestamps are epoch milliseconds.

export type BotStatus = 'idle' | 'working' | 'needsInput' | 'error'

export interface Bot {
  customIcon?: import('./custom-icons').CustomIcon
  id: string
  /** `agent`, or `group`: several bots and the user in one chat. */
  kind: string
  members: string[]
  name: string
  description: string
  avatarColor: string
  avatarShape: string
  backend: string
  command?: string | null
  cwd: string
  /** Host-allocated personal workspace rather than a user-selected project. */
  managedWorkspace: boolean
  /** `ask` or `auto`: whether Codync asks about what the agent asks, or approves it. */
  permission: string
  /** The agent mode it works in (Claude's Manual / Plan / Auto…): the agent's id, or `auto` for its Auto mode under
   *  any id. Null keeps the agent's default. */
  mode: string | null
  model?: string | null
  /** The model's own settings (effort, fast…): agent option id → value. */
  modelOptions: Record<string, string>
  /** The agent's Fast mode (Claude Code, Codex): faster replies for more usage. */
  fastMode: boolean
  /** What the agent says about Fast mode: `unknown` (no session yet), `offered`, `unsupported`,
   *  or `refused` (it turned it off; `fastModeIssue` says why). */
  fastModeSupport: string
  fastModeIssue: string | null
  /** Agents that carry on, in order, when the bot's own can't answer. */
  fallbacks: Fallback[]
  /** The bot may set its own fallbacks; null from an older host (no fallbacks there). */
  selfFallbacks: boolean | null
  /** The fallback the bot is on right now; null on its own agent. */
  agentStep: AgentStep | null
  /** Agents of its chain (its own included) resting now; one whose `until` passed isn't. */
  resting: Resting[]
  /** What it keeps running (`dev-servers.ts`). */
  devServers: DevServer[]
  pinned: boolean
  /** The computer's lead bot: the host makes the first, and the user may pick another (`setLead`) or delete it. Placed in the roster like any bot, never hidden. */
  lead: boolean
  /** Where the user dragged it, lowest first within its band (`rosterBand`); ties sort by recent activity. */
  position: number
  /** The roster section the user put it in (`Work`, `Personal`); none when absent. */
  section?: string
  hidden: boolean
  notify?: boolean | null
  connectors: string[]
  skills: string[]
  /** The bot can see and operate the computer's desktop. */
  computer: boolean
  /** The bot can create bots (which can too) and change any bot's setup (the built-in `manage` tools); null from an older host. */
  manageTeam: boolean | null
  /** Other computers in the user's networks list this bot and may message it; null from an older host. */
  offeredToNetwork: boolean | null
  /** The user's cloud assistant lists this bot and may message it; null from an older host. */
  reachableByAssistants: boolean | null
  /** The bot that created this one; null when the user did. */
  createdBy: string | null
  /** Uses the memory all the user's bots share ("About you"). */
  sharedMemory: boolean
  /** Keeps the harness's own memory (Claude Code's…) on beside Codync's; null where it has no switch. */
  harnessMemory: boolean | null
  /** The harness keeps one memory for all its sessions that Codync can't turn off (Hermes Agent, Gemini CLI): every bot on it shares it. */
  harnessMemoryShared: boolean
  /** When Codync summarizes the chat ahead of time; null where it doesn't (an agent it can't ask, an older host). */
  compaction: CompactionThreshold | null
  /** The sizes the chat's window allows; null until its agent has told the window. */
  compactionTokens: CompactionSizes | null
  /** Moves whenever memory changes, so an open memory screen fetches it again. */
  memoryRev: number
  autoName: boolean
  createdAt: number
  rev: number
  status: BotStatus | string
  activity: string
  startedAt?: number | null
  workingChat?: string | null
  unread: number
  lastMessage?: string | null
  lastAt: number
}

/** Lenient: a host that is a little newer or older than the app still yields a usable bot. */
export function normalizeBot(raw: Partial<Bot> & { id: string }): Bot {
  const createdAt = raw.createdAt ?? 0
  return {
    id: raw.id,
    kind: raw.kind ?? 'agent',
    members: raw.members ?? [],
    name: raw.name ?? 'Bot',
    description: raw.description ?? '',
    avatarColor: raw.avatarColor ?? 'blue',
    avatarShape: raw.avatarShape ?? 'blob',
    backend: raw.backend ?? 'custom',
    command: raw.command ?? null,
    cwd: raw.cwd ?? '',
    managedWorkspace: raw.managedWorkspace ?? false,
    permission: raw.permission ?? 'ask',
    mode: raw.mode ?? null,
    model: raw.model ?? null,
    modelOptions: raw.modelOptions ?? {},
    fastMode: raw.fastMode ?? false,
    fastModeSupport: raw.fastModeSupport ?? 'unknown',
    fastModeIssue: raw.fastModeIssue ?? null,
    fallbacks: raw.fallbacks ?? [],
    selfFallbacks: raw.selfFallbacks ?? null,
    agentStep: raw.agentStep ?? null,
    resting: raw.resting ?? [],
    devServers: (raw.devServers ?? []).map(normalizeDevServer),
    pinned: raw.pinned ?? false,
    lead: raw.lead ?? false,
    position: raw.position ?? 0,
    // A cleared section (null, or an empty name) reads as none.
    section: raw.section || undefined,
    hidden: raw.hidden ?? false,
    notify: raw.notify ?? null,
    connectors: raw.connectors ?? [],
    skills: raw.skills ?? [],
    computer: raw.computer ?? false,
    manageTeam: raw.manageTeam ?? null,
    offeredToNetwork: raw.offeredToNetwork ?? null,
    reachableByAssistants: raw.reachableByAssistants ?? null,
    createdBy: raw.createdBy ?? null,
    sharedMemory: raw.sharedMemory ?? raw.managedWorkspace ?? false,
    harnessMemory: raw.harnessMemory ?? null,
    harnessMemoryShared: raw.harnessMemoryShared ?? false,
    compaction: raw.compaction ?? null,
    compactionTokens: raw.compactionTokens ?? null,
    memoryRev: raw.memoryRev ?? 0,
    autoName: raw.autoName ?? false,
    createdAt,
    rev: raw.rev ?? 0,
    status: raw.status ?? 'idle',
    activity: raw.activity ?? '',
    startedAt: raw.startedAt ?? null,
    workingChat: raw.workingChat ?? null,
    unread: raw.unread ?? 0,
    lastMessage: raw.lastMessage ?? null,
    lastAt: raw.lastAt ?? createdAt,
  }
}

/** One step of a bot's fallback chain: another agent and the model to use on it. */
export interface Fallback {
  backend: string
  /** A custom command, only from the bot itself (the apps don't offer one). */
  command?: string
  model: string | null
  modelOptions: Record<string, string>
}

/** Why an agent can't answer: usageLimit · outOfCredits · rateLimited · overloaded · signedOut · notStarting · modelMissing */
export type FallbackReason = string

/** The fallback a bot runs on: which step, its agent and model, since when, and why it left the one before. */
export interface AgentStep {
  step: number
  agent: string
  model: string | null
  since: number
  reason: FallbackReason | null
}

/** An agent that can't answer until `until`. */
export interface Resting {
  agent: string
  until: number
  reason: FallbackReason
}

export const isGroup = (b: Bot) => b.kind === 'group'
export const isWorking = (b: Bot) => b.status === 'working' || b.status === 'needsInput'
export const needsInput = (b: Bot) => b.status === 'needsInput'
/** Working right now in this chat (its own, or a group it's in). */
export const isWorkingIn = (b: Bot, chat: string) => isWorking(b) && (b.workingChat ?? b.id) === chat
export const folderName = (b: Bot) =>
  b.managedWorkspace ? 'Personal workspace' : (b.cwd.split('/').filter(Boolean).pop() ?? b.cwd)

/** Create / update payload for a bot. */
/** When Codync summarizes a bot's chat ahead of time: a preset, or at a size in tokens. */
export type CompactionThreshold = 'early' | 'nearLimit' | 'off' | number

/** The sizes a chat's window allows: `min`..`max` by `step` tokens, and the few a menu lists. */
export interface CompactionSizes {
  min: number
  max: number
  step: number
  presets: number[]
}

export interface BotDraft {
  id?: string
  name: string
  description: string
  avatarColor: string
  avatarShape: string
  backend: string
  command?: string | null
  cwd: string
  permission: string
  /** Null for the agent's default: sent as is, so the host clears it. */
  mode?: string | null
  model?: string | null
  modelOptions?: Record<string, string>
  fastMode?: boolean
  fallbacks?: Fallback[]
  selfFallbacks?: boolean
  pinned?: boolean
  /** A name puts the bot in that roster section, null takes it out; left out, the host keeps it. */
  section?: string | null
  hidden?: boolean
  notify?: boolean | null
  connectors?: string[]
  skills?: string[]
  computer?: boolean
  manageTeam?: boolean
  offeredToNetwork?: boolean
  reachableByAssistants?: boolean
  sharedMemory?: boolean
  harnessMemory?: boolean
  /** Left out where the bot has none: the host keeps what it has. */
  compaction?: CompactionThreshold
}

export function draftOf(bot: Bot): BotDraft {
  return {
    id: bot.id,
    name: bot.name,
    description: bot.description,
    avatarColor: bot.avatarColor,
    avatarShape: bot.avatarShape,
    backend: bot.backend,
    command: bot.command ?? null,
    cwd: bot.managedWorkspace ? '' : bot.cwd,
    permission: bot.permission,
    mode: bot.mode,
    model: bot.model ?? null,
    modelOptions: bot.modelOptions,
    fastMode: bot.fastMode,
    fallbacks: bot.fallbacks,
    selfFallbacks: bot.selfFallbacks ?? undefined,
    pinned: bot.pinned,
    hidden: bot.hidden,
    notify: bot.notify ?? null,
    connectors: bot.connectors,
    skills: bot.skills,
    computer: bot.computer,
    manageTeam: bot.manageTeam ?? undefined,
    offeredToNetwork: bot.offeredToNetwork ?? undefined,
    reachableByAssistants: bot.reachableByAssistants ?? undefined,
    compaction: bot.compaction ?? undefined,
  }
}

export interface GroupDraft {
  id?: string
  kind: 'group'
  name: string
  description: string
  members: string[]
  pinned?: boolean
}

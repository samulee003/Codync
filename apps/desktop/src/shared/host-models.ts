export interface UsageWindow {
  id: string
  label: string
  percent: number
  resetsAt?: number | null
  resetsText?: string | null
  /** A reading that isn't a share of a cap. Shown instead of the percent, and never the tightest limit. */
  text?: string | null
}

export interface UsageProvider {
  id: string
  name: string
  windows: UsageWindow[]
  source: string
  updatedAt: number
  /** The agent's ACP registry id, which its logo goes by. Absent from a host that doesn't name it. */
  registry?: string
}

/** An agent the Usage screen can show or hide. The host keeps the choice, so every app follows it. */
export interface UsageService {
  id: string
  name: string
  shown: boolean
  /** The agent's ACP registry id, which its logo goes by. */
  registry?: string
}

export interface Usage {
  providers: UsageProvider[]
  /** Absent from a host older than the chooser. */
  services?: UsageService[]
}

export interface Backend {
  id: string
  name: string
  available: boolean
  command: string
  installHint: string
  registry?: string
  description?: string
  installed?: boolean
  curated?: boolean
  signedIn?: boolean
  canInstall?: boolean
  /** Only for an agent that runs on Node.js, on a computer without its own: Codync's copy is missing or added. */
  node?: 'missing' | 'added' | null
}

export interface ScreenDisplay {
  id: number
  name: string
  width: number
  height: number
  main: boolean
}

export interface ScreenState {
  /** The helper's display name in operating-system permission settings. */
  permissionApp?: string
  enabled: boolean
  connected: boolean
  platform: string
  capture: boolean
  input: boolean
  displays: ScreenDisplay[]
  userControl: boolean
  agentBot?: string | null
  /** Every bot using the computer right now (several can at once); older hosts send only `agentBot`. */
  agentBots?: string[]
  /** Bots with computer use can act here (Windows: always; elsewhere Remote screen is on). */
  computerUse: boolean
  viewers: number
  /** Which driver runs the bots' computer tools (Settings → Computer access); undefined from hosts before the choice. */
  driver?: ComputerDriver
  /** The host's environment forces agent-desktop (`CODYNC_COMPUTER_DRIVER`), whatever `driver` says. */
  driverForced?: boolean
  /** The agent-desktop CLI is installed; without it the default driver runs. */
  agentDesktopInstalled?: boolean
}

/** `default`: cua-driver, one bot at a time. `agentDesktop`: several bots at once, in the background. */
export type ComputerDriver = 'default' | 'agentDesktop'

export function normalizeScreen(raw: Partial<ScreenState> | undefined | null): ScreenState | null {
  if (!raw) return null
  return {
    enabled: raw.enabled ?? false,
    connected: raw.connected ?? false,
    platform: raw.platform ?? '',
    permissionApp: raw.permissionApp,
    capture: raw.capture ?? false,
    input: raw.input ?? false,
    displays: raw.displays ?? [],
    userControl: raw.userControl ?? false,
    agentBot: raw.agentBot ?? null,
    agentBots: raw.agentBots ?? (raw.agentBot ? [raw.agentBot] : []),
    computerUse: raw.computerUse ?? false,
    viewers: raw.viewers ?? 0,
    driver: raw.driver,
    driverForced: raw.driverForced,
    agentDesktopInstalled: raw.agentDesktopInstalled,
  }
}

export interface Hello {
  iconsRevision?: number
  icons?: import('./custom-icons').CustomIcons
  hostId: string
  name: string
  version: string
  os: string
  device?: string
  home?: string
  backends: Backend[]
  rev: number
  screen?: ScreenState
  urls?: string[]
  computerId?: string
  signKey?: string
  boxKey?: string
  minApp?: string
  /** `installHostUpdate` can update this computer (absent before hosts could be updated from a client). */
  canUpdate?: boolean
  /** The latest release the computer found; null while unknown (development builds, Homebrew, Windows; absent from older hosts). */
  latestVersion?: string | null
  cloud?: string | null
  /** Product analytics on this computer: on unless its owner turned it off. */
  analytics?: boolean
}

export interface PairingInfo {
  pairingUrl: string
  urls: string[]
  expiresAt?: number
}

export interface DirItem {
  name: string
  path: string
  isGit: boolean
}

export interface RecentWorkspace { path: string; usedAt: number }

export interface DirListing {
  recent?: RecentWorkspace[]
  ancestors?: DirItem[]
  path: string
  parent?: string | null
  isGit: boolean
  dirs: DirItem[]
}

/** A pending request as the computer shows it (loopback `accessRequests`). */
export interface AccessRequest {
  requestId: string
  deviceKey: string
  deviceName: string
  platform?: string | null
  email?: string | null
  /** The SAS; nil until the phone revealed its nonce (Approve stays disabled). */
  code?: string | null
  createdAt?: number | null
  expiresAt?: number | null
}

/** A signed-in phone that scanned a computer in no account, waiting to be let in (loopback `pairingRequests`). */
export interface PairingRequest {
  requestId: string
  deviceName: string
  platform?: string | null
  /** The phone's account, which the computer joins. */
  email?: string | null
  /** The 6 digits the phone shows too. */
  code: string
  expiresAt?: number | null
}

/** A device the host authorized (loopback `devices`). */
export interface AuthorizedDevice {
  key: string
  name: string
  platform?: string | null
  /** `local` (QR) or `account`. */
  source: string
  scopes?: string[]
  leaseUntil?: number | null
  createdAt?: number | null
  lastSeenAt?: number | null
  connected?: boolean | null
}

export interface CloudOwner {
  userId: string
  email?: string | null
}

/** `code`: compare the 6-digit code at the computer; `auto`: let account devices in. */
export type AccountApproval = 'code' | 'auto'

export interface CloudStatus {
  enabled: boolean
  url?: string | null
  /** A private relay's registration token is saved (the token itself never leaves the host). */
  tokenSet?: boolean | null
  registered?: boolean | null
  connected?: boolean | null
  owner?: CloudOwner | null
  lastError?: string | null
  approval?: AccountApproval | null
  /** Whose relay: a `private` one has no account, access requests or relayed screen. Absent from older hosts (always Codync's). */
  relay?: 'codync' | 'private' | null
  /** The user's cloud assistant may message this computer's bots (`setAssistants`). */
  assistants?: boolean | null
  /** In no account while it could join one: a signed-in phone that pairs asks to bring it into its account. Absent from older hosts. */
  canJoinAccount?: boolean | null
  /** Turned on, the cloud would leave it in no account on a Codync cloud; known with the cloud off too. Absent from older hosts, which just turn on. */
  joinsAccountOnEnable?: boolean | null
}

export interface ClaimSignature {
  computerId: string
  signKey: string
  boxKey: string
  name: string
  platform: string
  device?: string | null
  version: string
  sig: string
}

/** A computer this device talks to. Holds no secrets: only pinned public keys and addresses. */
export interface Computer {
  id: string
  name: string
  signKey: string
  boxKey?: string | null
  urls: string[]
  cloud?: string | null
  device?: string | null
  color?: string | null
}

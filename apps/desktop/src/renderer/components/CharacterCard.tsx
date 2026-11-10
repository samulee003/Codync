import { staminaLevel, type Stamina } from '@shared/models'
import { ABILITIES, chartPoints, classInfo, levelProgress, type BotClass, type BotSheet } from '@shared/sheet'
import { useTr } from '../lib/use-language'
import { ClassPicker } from './ClassPicker'
import { ProgressBar } from './Controls'
import { Icon } from './Icon'
import { HStack, VStack } from './Stack'
import { Text } from './Text'
import './character-card.css'

const CHART = { size: 132, center: 66, radius: 50 }
const RINGS = [25, 50, 75, 100]

/**
 * A bot's character sheet, laid out like a unit's status screen in a tactics game: class and level, stamina (what is
 * left of its agent's plan), an ability chart with the five numbers, and what it has done on the team board.
 */
export function CharacterCard({ sheet, stamina, setClass, disabled }: {
  sheet: BotSheet
  stamina: Stamina | undefined
  setClass: (id: BotClass | null) => void
  disabled?: boolean
}) {
  const tr = useTr()
  const info = classInfo(sheet.class)
  const values = ABILITIES.map((a) => sheet.abilities[a.id])
  const full = ABILITIES.map(() => 100)
  return (
    <VStack gap={12} className="character-card">
      <HStack gap={10}>
        <span className="character-emblem"><Icon name={info.symbol} size={18} color="var(--accent)" /></span>
        <VStack gap={2} grow>
          <HStack gap={6} align="baseline">
            <Text size="body" weight="semibold">{tr(info.title)}</Text>
            <span className="character-level">{tr('Lv {level}').replace('{level}', String(sheet.level))}</span>
          </HStack>
          <ProgressBar value={levelProgress(sheet) / 100} label={tr('Experience')} fill="var(--accent)" className="character-xp" />
          <Text size="caption" tone="secondary">{tr('{xp} / {next} XP').replace('{xp}', String(sheet.xp)).replace('{next}', String(sheet.nextLevelXp))}</Text>
        </VStack>
      </HStack>

      {stamina ? (
        <HStack gap={8}>
          <Text size="caption" weight="semibold" className="character-stat-name">{tr('Stamina')}</Text>
          <ProgressBar value={stamina.left / 100} label={tr('Stamina')} className={`character-bar stamina-${staminaLevel(stamina.left)}`} />
          <Text size="caption" mono className="character-stat-value">{stamina.left}</Text>
        </HStack>
      ) : null}

      <HStack gap={12} align="center">
        <svg className="character-chart" width={CHART.size} height={CHART.size} viewBox={`0 0 ${CHART.size} ${CHART.size}`} role="img" aria-label={tr('Abilities')}>
          {RINGS.map((ring) => (
            <polygon key={ring} className="character-chart-ring" points={chartPoints(full.map(() => ring), CHART.center, CHART.radius)} />
          ))}
          <polygon className="character-chart-shape" points={chartPoints(values, CHART.center, CHART.radius)} />
        </svg>
        <VStack gap={6} grow>
          {ABILITIES.map((a) => (
            <HStack key={a.id} gap={8}>
              <Text size="caption" weight="semibold" className="character-stat-name">{tr(a.title)}</Text>
              <ProgressBar value={sheet.abilities[a.id] / 100} label={tr(a.title)} fill="var(--accent)" className="character-bar" />
              <Text size="caption" mono className="character-stat-value">{sheet.abilities[a.id]}</Text>
            </HStack>
          ))}
        </VStack>
      </HStack>

      <Text size="caption" tone="secondary">
        {[
          tr('Finished {count}').replace('{count}', String(sheet.deeds.finished)),
          tr('First try {count}').replace('{count}', String(sheet.deeds.firstTry)),
          tr('Checked {count}').replace('{count}', String(sheet.deeds.checked)),
          tr('Planned {count}').replace('{count}', String(sheet.deeds.planned)),
        ].join(' · ')}
      </Text>

      <ClassPicker selection={sheet.class} onChange={setClass} disabled={disabled} />
      <Text size="caption" tone="secondary">{tr('A class sets where abilities start; they grow with what the bot does on the team board.')}</Text>
    </VStack>
  )
}

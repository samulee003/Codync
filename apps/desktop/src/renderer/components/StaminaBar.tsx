import { staminaLevel, type Stamina } from '@shared/models'
import { useResetLine, useTr } from '../lib/use-language'
import { resetDescription } from '../views/usage/usage-model'
import './stamina-bar.css'

/**
 * A game-style stamina bar under a bot's avatar: how much of its agent's plan is left (the tightest limit), green while
 * plenty, yellow under half, red when nearly out. The tooltip names the limit and when it resets.
 */
export function StaminaBar({ stamina, width }: { stamina: Stamina | undefined; width: number }) {
  const tr = useTr()
  const resetLine = useResetLine()
  if (!stamina) return null
  const reset = resetDescription({ id: '', label: stamina.label, percent: 100 - stamina.left, resetsAt: stamina.resetsAt, resetsText: stamina.resetsText })
  const title = [tr(stamina.label), tr('{percent}% left').replace('{percent}', String(stamina.left)), reset ? resetLine(reset) : null]
    .filter(Boolean).join(' · ')
  return (
    <span className={`stamina-bar ${staminaLevel(stamina.left)}`} style={{ width }} title={title} role="meter" aria-label={title}
      aria-valuemin={0} aria-valuemax={100} aria-valuenow={stamina.left}>
      <span className="stamina-fill" style={{ width: `${stamina.left}%` }} />
    </span>
  )
}

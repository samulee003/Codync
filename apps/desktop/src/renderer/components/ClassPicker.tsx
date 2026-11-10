import { BOT_CLASSES, type BotClass } from '@shared/sheet'
import { useTr } from '../lib/use-language'
import { Icon } from './Icon'
import { Text } from './Text'
import './character-card.css'

/** A bot's class, picked like a unit's in a tactics game: one tile per class, its emblem, name and what it is for. */
export function ClassPicker({ selection, onChange, disabled }: { selection: BotClass | null; onChange: (id: BotClass | null) => void; disabled?: boolean }) {
  const tr = useTr()
  return (
    <div className="class-picker" role="radiogroup" aria-label={tr('Class')}>
      {BOT_CLASSES.map((c) => {
        const on = c.id === selection
        return (
          <button key={c.id} type="button" role="radio" aria-checked={on} disabled={disabled} title={tr(c.role)}
            className={`press class-tile${on ? ' on' : ''}`} onClick={() => onChange(on ? null : c.id)}>
            <Icon name={c.symbol} size={16} color={on ? 'var(--accent)' : 'var(--secondary)'} />
            <Text size="caption" weight={on ? 'semibold' : 'medium'} tone={on ? 'text' : 'secondary'} truncate>{tr(c.titles[0])}</Text>
          </button>
        )
      })}
    </div>
  )
}

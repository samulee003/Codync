import { useState } from 'react'
import { Button, Switch } from '../../components/Controls'
import { Icon } from '../../components/Icon'
import { Sheet } from '../../components/Overlay'
import { HStack, VStack } from '../../components/Stack'
import { Text } from '../../components/Text'
import { isGroup } from '@shared/models'
import { useTr } from '../../lib/use-language'
import { useStore } from '../../store/context'
import { FolderPicker } from '../bots/FolderPicker'

type Run = (work: Promise<unknown>) => void

/** A new task: its title, what to do, and the folder it's in, picked like a bot's workspace. */
export function NewTaskForm({ done, run }: { done: () => void; run: Run }) {
  const tr = useTr()
  const store = useStore()
  const [title, setTitle] = useState('')
  const [detail, setDetail] = useState('')
  const [project, setProject] = useState<string | null>(null)
  const [picking, setPicking] = useState(false)
  const add = () => {
    if (!title.trim()) return
    run(store.board.create(store.client, title, detail, project).then(() => {
      setTitle('')
      setDetail('')
      setProject(null)
      done()
    }))
  }
  return (
    <VStack gap={8} className="outlined-card board-form">
      <input className="board-input" autoFocus maxLength={200} value={title} placeholder={tr('Task title')} aria-label={tr('Task title')}
        onChange={(e) => setTitle(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && add()} />
      <textarea className="board-input" value={detail} placeholder={tr('What to do, and how to check it is done')} onChange={(e) => setDetail(e.target.value)} />
      <HStack gap={8}>
        <button className="board-input board-folder" onClick={() => setPicking(true)}>
          <Icon name="folder" size={14} color="var(--secondary)" />
          <Text size="callout" tone={project ? 'text' : 'secondary'} truncate grow mono={!!project}>{project ?? tr('Project folder (optional)')}</Text>
        </button>
        {project ? <Button kind="secondary" onClick={() => setProject(null)}>{tr('Clear')}</Button> : null}
      </HStack>
      <HStack gap={8} justify="end">
        <Button kind="secondary" onClick={done}>{tr('Cancel')}</Button>
        <Button disabled={!title.trim()} onClick={add}>{tr('Add task')}</Button>
      </HStack>
      <Sheet open={picking} onClose={() => setPicking(false)} width={460} height={520}>
        <FolderPicker start={project ?? store.hello?.home ?? null} onPick={(path) => { setProject(path); setPicking(false) }} />
      </Sheet>
    </VStack>
  )
}

/** A whole goal, for the lead bot to split into tasks the bots then claim. */
export function SplitGoalForm({ done, run }: { done: (splitter: string) => void; run: Run }) {
  const tr = useTr()
  const store = useStore()
  const [goal, setGoal] = useState('')
  const split = () => {
    if (!goal.trim()) return
    run(store.board.split(store.client, goal).then((name) => {
      setGoal('')
      done(name)
    }))
  }
  return (
    <VStack gap={8} className="outlined-card board-form">
      <Text size="callout" tone="secondary">{tr('Your lead bot splits it into tasks, and the bots claim them.')}</Text>
      <textarea className="board-input" autoFocus value={goal} placeholder={tr('The goal, e.g. prepare both kids’ exam review papers')} onChange={(e) => setGoal(e.target.value)} />
      <HStack gap={8} justify="end">
        <Button kind="secondary" onClick={() => done('')}>{tr('Cancel')}</Button>
        <Button disabled={!goal.trim()} onClick={split}>{tr('Split into tasks')}</Button>
      </HStack>
    </VStack>
  )
}

/** Which bots come for open tasks on their own: all, unless turned off here. */
export function BoardMembers({ run }: { run: Run }) {
  const tr = useTr()
  const store = useStore()
  const bots = [...store.bots.values()].filter((b) => !b.hidden && !isGroup(b)).sort((a, b) => a.name.localeCompare(b.name))
  if (!bots.length) return null
  return (
    <section className="details-section">
      <Text className="details-section-title">{tr('Bots that claim tasks on their own')}</Text>
      <VStack gap={2} className="outlined-card board-members">
        {bots.map((bot) => (
          <Switch key={bot.id} on={!store.board.optedOut.has(bot.id)} label={bot.name} onChange={(on) => run(store.board.setClaims(store.client, bot.id, on))}>
            <Text size="callout">{bot.name}</Text>
          </Switch>
        ))}
      </VStack>
    </section>
  )
}

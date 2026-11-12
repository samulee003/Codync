import { useState } from 'react'
import { Button, DropdownMenu, Switch } from '../../components/Controls'
import { Icon } from '../../components/Icon'
import { Sheet } from '../../components/Overlay'
import type { MenuItem } from '../../components/Menu'
import { HStack, VStack } from '../../components/Stack'
import { Text } from '../../components/Text'
import { parseCriteria, type TaskTemplate } from '@shared/board'
import { isGroup } from '@shared/models'
import { useTr } from '../../lib/use-language'
import { useStore } from '../../store/context'
import { FolderPicker } from '../bots/FolderPicker'

type Run = (work: Promise<unknown>) => void

/**
 * A new task as a work order: its title, what to do, the acceptance criteria the check holds it to, whether its plan
 * waits for the user's approval before any work, and the folder it's in, picked like a bot's workspace.
 */
export function NewTaskForm({ done, run }: { done: () => void; run: Run }) {
  const tr = useTr()
  const store = useStore()
  const [title, setTitle] = useState('')
  const [detail, setDetail] = useState('')
  const [criteria, setCriteria] = useState('')
  const [approvePlan, setApprovePlan] = useState(true)
  const [project, setProject] = useState<string | null>(null)
  const [picking, setPicking] = useState(false)
  const applyTemplate = (t: TaskTemplate) => {
    setTitle(t.title)
    setDetail(t.detail)
    setCriteria(t.acceptance.join('\n'))
    setApprovePlan(t.approvePlan)
    setProject(t.project)
  }
  const templateItems = (): MenuItem[] => {
    const { templates } = store.board
    return [
      ...templates.map((t): MenuItem => ({ title: t.name, verbatim: true, action: () => applyTemplate(t) })),
      ...templates.map((t, i): MenuItem => ({
        title: tr('Delete template: {name}').replace('{name}', t.name),
        verbatim: true,
        icon: 'trash',
        destructive: true,
        divider: i === 0,
        action: () => run(store.board.deleteTemplate(store.client, t.id)),
      })),
    ]
  }
  const add = () => {
    if (!title.trim()) return
    run(store.board.create(store.client, title, detail, project, parseCriteria(criteria), approvePlan).then(() => {
      setTitle('')
      setDetail('')
      setCriteria('')
      setProject(null)
      done()
    }))
  }
  return (
    <VStack gap={8} className="outlined-card board-form">
      {store.board.templates.length > 0 ? (
        <DropdownMenu className="board-input board-template" title={tr('Template')} items={templateItems}>
          <Icon name="doc.on.doc" size={14} color="var(--secondary)" />
          <Text size="callout" tone="secondary" grow>{tr('Template')}</Text>
        </DropdownMenu>
      ) : null}
      <input className="board-input" autoFocus maxLength={200} value={title} placeholder={tr('Task title')} aria-label={tr('Task title')}
        onChange={(e) => setTitle(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && add()} />
      <textarea className="board-input" value={detail} placeholder={tr('What to do: the result, the materials to use, and what not to do')} onChange={(e) => setDetail(e.target.value)} />
      <textarea className="board-input" value={criteria} placeholder={tr('Acceptance criteria, one per line: what must hold for it to count as done')}
        aria-label={tr('Acceptance criteria')} onChange={(e) => setCriteria(e.target.value)} />
      <Switch on={approvePlan} onChange={setApprovePlan} label="Approve the plan before work starts">
        <Text size="callout">{tr('Approve the plan before work starts')}</Text>
      </Switch>
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

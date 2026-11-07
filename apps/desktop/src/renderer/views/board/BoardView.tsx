import { useEffect, useState } from 'react'
import { boardColumns, taskActions, type BoardTask, type TaskAction } from '@shared/board'
import { Button, DropdownMenu, Reveal } from '../../components/Controls'
import { BotAvatar } from '../../components/Avatar'
import { Icon } from '../../components/Icon'
import { IconButtonBase, ModalHeader, SheetPage, type MenuItem } from '../../components/Overlay'
import { HStack, VStack } from '../../components/Stack'
import { Text } from '../../components/Text'
import { isGroup } from '@shared/models'
import { useTr } from '../../lib/use-language'
import { useScreenViewed } from '../../store/analytics'
import { useStore } from '../../store/context'
import type { BotStore } from '../../store/bot-store'
import './board.css'

/**
 * The computer's team board (`host/src/board/`): tasks the user and the bots share, by status. The user adds tasks,
 * hands one to a bot, merges a finished task's branch, and reopens, closes or deletes tasks; bots claim and finish them
 * through their own tools.
 */
export function BoardView() {
  const tr = useTr()
  const store = useStore()
  const [adding, setAdding] = useState(false)
  const [error, setError] = useState<string | null>(null)
  useScreenViewed('board')
  useEffect(() => {
    store.board.load(store.client).then(() => setError(null), (e: unknown) => setError(message(e)))
  }, [store, store.client])
  const run = (work: Promise<unknown>) => work.then(() => setError(null), (e: unknown) => setError(message(e)))
  const columns = boardColumns(store.board.byId.values())
  const empty = columns.every((c) => c.tasks.length === 0)
  return (
    <SheetPage className="board">
      <ModalHeader
        title={tr('Team board')}
        subtitle={tr('Tasks your bots split, claim and finish together')}
        icon="checklist"
        trailing={<IconButtonBase title={tr('New task')} icon="plus" selected={adding} onClick={() => setAdding((a) => !a)} />}
      />
      <VStack gap={16} className="board-body">
        <Reveal show={adding}>
          <NewTaskForm done={() => setAdding(false)} run={run} />
        </Reveal>
        {error ? <Text size="caption" tone="danger" role="alert">{error}</Text> : null}
        {empty && !adding ? (
          <VStack gap={4} className="outlined-card board-empty">
            <Text size="callout" weight="semibold">{tr('No tasks yet')}</Text>
            <Text size="callout" tone="secondary">{tr('Add a task with +, or ask a bot to split a job into tasks.')}</Text>
          </VStack>
        ) : null}
        {columns.filter((c) => c.tasks.length > 0).map((column) => (
          <section key={column.status} className="details-section">
            <Text className="details-section-title">{`${tr(column.title)} · ${column.tasks.length}`}</Text>
            <VStack gap={6}>
              {column.tasks.map((task) => <TaskRow key={task.id} task={task} run={run} />)}
            </VStack>
          </section>
        ))}
      </VStack>
    </SheetPage>
  )
}

function NewTaskForm({ done, run }: { done: () => void; run: (work: Promise<unknown>) => void }) {
  const tr = useTr()
  const store = useStore()
  const [title, setTitle] = useState('')
  const [detail, setDetail] = useState('')
  const [project, setProject] = useState('')
  const add = () => {
    if (!title.trim()) return
    run(store.board.create(store.client, title, detail, project.trim() || null).then(() => {
      setTitle('')
      setDetail('')
      done()
    }))
  }
  return (
    <VStack gap={8} className="outlined-card board-form">
      <input className="board-input" autoFocus maxLength={200} value={title} placeholder={tr('Task title')} aria-label={tr('Task title')}
        onChange={(e) => setTitle(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && add()} />
      <textarea className="board-input" value={detail} placeholder={tr('What to do, and how to check it is done')} onChange={(e) => setDetail(e.target.value)} />
      <input className="board-input board-mono" value={project} placeholder={tr('Project folder (optional)')} aria-label={tr('Project folder (optional)')} onChange={(e) => setProject(e.target.value)} />
      <HStack gap={8} justify="end">
        <Button kind="secondary" onClick={done}>{tr('Cancel')}</Button>
        <Button disabled={!title.trim()} onClick={add}>{tr('Add task')}</Button>
      </HStack>
    </VStack>
  )
}

const STATUS_ICON: Record<BoardTask['status'], string> = {
  open: 'circle',
  claimed: 'circle.dotted',
  review: 'arrow.triangle.merge',
  done: 'checkmark.circle.fill',
  cancelled: 'xmark.circle',
}

function TaskRow({ task, run }: { task: BoardTask; run: (work: Promise<unknown>) => void }) {
  const tr = useTr()
  const store = useStore()
  const [open, setOpen] = useState(false)
  const [note, setNote] = useState('')
  const claimer = task.claimedBy ? store.bots.get(task.claimedBy)?.name ?? task.claimedBy : null
  const addNote = () => {
    if (!note.trim()) return
    run(store.board.note(store.client, task.id, note).then(() => setNote('')))
  }
  return (
    <VStack gap={8} className="outlined-card board-task">
      <HStack gap={10}>
        <button className="board-task-main" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
          <Icon name={STATUS_ICON[task.status]} size={14} color="var(--secondary)" />
          <VStack gap={2} grow>
            <Text size="body" weight="medium" lines={2}>{task.title}</Text>
            <Text size="caption" tone="secondary" truncate>
              {[claimer, task.branch, task.notes.length ? tr('{count} notes').replace('{count}', String(task.notes.length)) : null].filter(Boolean).join(' · ')}
            </Text>
          </VStack>
        </button>
        <DropdownMenu title={tr('Task actions')} className="board-task-menu" items={() => menuItems(task, store, tr, run)}>
          <Icon name="ellipsis" size={14} color="var(--secondary)" />
        </DropdownMenu>
      </HStack>
      <Reveal show={open}>
        <VStack gap={8} className="board-task-detail">
          {task.detail ? <Text size="callout" className="board-text">{task.detail}</Text> : null}
          {task.project ? <Text size="caption" tone="secondary" mono truncate>{task.worktree ?? task.project}</Text> : null}
          {task.summary ? <Text size="callout" className="board-text">{task.summary}</Text> : null}
          {task.notes.map((n) => (
            <Text key={n.at} size="callout" tone="secondary" className="board-text">
              {`${n.by ? store.bots.get(n.by)?.name ?? n.by : tr('You')}: ${n.text}`}
            </Text>
          ))}
          <input className="board-input" value={note} maxLength={4000} placeholder={tr('Add a note for the team')} aria-label={tr('Add a note for the team')}
            onChange={(e) => setNote(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && addNote()} />
        </VStack>
      </Reveal>
    </VStack>
  )
}

const ACTION: Record<Exclude<TaskAction, 'assign'>, { title: string; icon: string; destructive?: boolean }> = {
  merge: { title: 'Merge branch', icon: 'arrow.triangle.merge' },
  reopen: { title: 'Reopen', icon: 'arrow.uturn.backward' },
  done: { title: 'Mark done', icon: 'checkmark' },
  cancel: { title: 'Cancel task', icon: 'xmark' },
  delete: { title: 'Delete task', icon: 'trash', destructive: true },
}

/** A task's menu: one item per action; assigning lists this computer's bots, each its own item. */
function menuItems(task: BoardTask, store: BotStore, tr: (text: string) => string, run: (work: Promise<unknown>) => void): MenuItem[] {
  const board = store.board
  const perform: Record<Exclude<TaskAction, 'assign'>, () => Promise<unknown>> = {
    merge: () => board.merge(store.client, task.id),
    reopen: () => board.setStatus(store.client, task.id, 'open'),
    done: () => board.setStatus(store.client, task.id, 'done'),
    cancel: () => board.setStatus(store.client, task.id, 'cancelled'),
    delete: () => board.delete(store.client, task.id),
  }
  return taskActions(task).flatMap((action): MenuItem[] => {
    if (action === 'assign') {
      return [...store.bots.values()].filter((b) => !b.hidden && !isGroup(b)).map((bot) => ({
        title: tr('Assign to {name}').replace('{name}', bot.name),
        verbatim: true,
        leading: <BotAvatar bot={bot} size={18} animated={false} />,
        action: () => run(board.assign(store.client, task.id, bot.id)),
      }))
    }
    const { title, icon, destructive } = ACTION[action]
    return [{ title, icon, destructive, action: () => run(perform[action]()) }]
  })
}

function message(e: unknown) {
  return e instanceof Error ? e.message : String(e)
}

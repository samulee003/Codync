import { useState } from 'react'
import { awaitsApproval, criteriaWithChecks, type BoardTask } from '@shared/board'
import { Button } from '../../components/Controls'
import { Icon } from '../../components/Icon'
import { HStack, VStack } from '../../components/Stack'
import { Text } from '../../components/Text'
import { useTr } from '../../lib/use-language'
import { useStore } from '../../store/context'

type Run = (work: Promise<unknown>) => void

/**
 * A task's work order in its card: the acceptance criteria, each with the latest check's verdict, and the claimer's
 * plan; while the plan waits for the user, approving it or sending it back with what to change.
 */
export function TaskWorkOrder({ task, run }: { task: BoardTask; run: Run }) {
  const tr = useTr()
  const criteria = criteriaWithChecks(task)
  return (
    <VStack gap={8}>
      {criteria.length ? (
        <VStack gap={4}>
          <Text size="caption" weight="semibold" tone="secondary">{tr('Acceptance criteria')}</Text>
          {criteria.map(({ criterion, check }) => (
            <HStack key={criterion} gap={6} align="start">
              <Icon name={check ? (check.pass ? 'checkmark.circle.fill' : 'xmark.circle.fill') : 'circle'} size={13}
                color={check ? (check.pass ? 'var(--added)' : 'var(--danger)') : 'var(--secondary)'} title={check ? tr(check.pass ? 'Passed' : 'Failed') : tr('Not checked yet')} />
              <VStack gap={2} grow>
                <Text size="callout" className="board-text">{criterion}</Text>
                {check?.note ? <Text size="caption" tone="secondary" className="board-text">{check.note}</Text> : null}
              </VStack>
            </HStack>
          ))}
        </VStack>
      ) : null}
      {task.plan ? (
        <VStack gap={4}>
          <Text size="caption" weight="semibold" tone="secondary">{tr(task.approval === 'approved' ? 'Approved plan' : 'Proposed plan')}</Text>
          <Text size="callout" className="board-text">{task.plan}</Text>
        </VStack>
      ) : null}
      {awaitsApproval(task) ? <PlanDecision task={task} run={run} /> : null}
    </VStack>
  )
}

/** Approve the proposed plan, or send it back saying what to change. */
function PlanDecision({ task, run }: { task: BoardTask; run: Run }) {
  const tr = useTr()
  const store = useStore()
  const [change, setChange] = useState('')
  const revise = () => {
    if (!change.trim()) return
    run(store.board.revisePlan(store.client, task.id, change).then(() => setChange('')))
  }
  return (
    <VStack gap={6}>
      <input className="board-input" value={change} maxLength={4000} placeholder={tr('What to change in the plan')} aria-label={tr('What to change in the plan')}
        onChange={(e) => setChange(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && revise()} />
      <HStack gap={8} justify="end">
        <Button kind="secondary" disabled={!change.trim()} onClick={revise}>{tr('Ask for changes')}</Button>
        <Button onClick={() => run(store.board.approvePlan(store.client, task.id))}>{tr('Approve plan')}</Button>
      </HStack>
    </VStack>
  )
}

import { useState } from 'react'
import { awaitsAnswers, type BoardTask } from '@shared/board'
import { Button } from '../../components/Controls'
import { HStack, VStack } from '../../components/Stack'
import { Text } from '../../components/Text'
import { useTr } from '../../lib/use-language'
import { useStore } from '../../store/context'

type Run = (work: Promise<unknown>) => void

/**
 * The questions a claimer asked before planning: answered in inputs while the task waits for the user, then shown
 * with each answer as text.
 */
export function TaskQuestions({ task, run }: { task: BoardTask; run: Run }) {
  const tr = useTr()
  const questions = task.questions ?? []
  if (!questions.length) return null
  return (
    <VStack gap={4}>
      <Text size="caption" weight="semibold" tone="secondary">{tr('Questions for you')}</Text>
      {awaitsAnswers(task) ? <AnswerForm task={task} run={run} /> : questions.map((q) => (
        <VStack key={q.text} gap={2}>
          <Text size="callout">{q.text}</Text>
          {q.answer ? <Text size="callout" tone="secondary" className="board-text">{q.answer}</Text> : null}
        </VStack>
      ))}
    </VStack>
  )
}

/** One input per question, sent together once each has an answer (as on the phones). */
function AnswerForm({ task, run }: { task: BoardTask; run: Run }) {
  const tr = useTr()
  const store = useStore()
  const questions = task.questions ?? []
  const [answers, setAnswers] = useState<string[]>(() => questions.map(() => ''))
  const complete = questions.every((_, i) => (answers[i] ?? '').trim())
  const send = () => {
    if (!complete) return
    run(store.board.answerQuestions(store.client, task.id, answers.map((a) => a.trim())))
  }
  return (
    <VStack gap={6}>
      {questions.map((q, i) => (
        <VStack key={q.text} gap={4}>
          <Text size="callout">{q.text}</Text>
          <input className="board-input" value={answers[i] ?? ''} maxLength={4000} placeholder={tr('Your answer')} aria-label={q.text}
            onChange={(e) => setAnswers((prev) => prev.map((a, j) => (j === i ? e.target.value : a)))} />
        </VStack>
      ))}
      <HStack gap={8} justify="end">
        <Button disabled={!complete} onClick={send}>{tr('Send answers')}</Button>
      </HStack>
    </VStack>
  )
}

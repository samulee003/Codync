import { useState } from 'react'
import { hasBoard } from '@shared/board'
import { Sheet } from '../../components/Overlay'
import type { AppModel } from '../../store/app-model'
import type { BotStore } from '../../store/bot-store'
import { StoreContext } from '../../store/context'
import { BoardView } from './BoardView'

/**
 * The team board's sheet and what opens it: the board of the selected bot's computer, else this computer's, else
 * the first online one with a host that has a board. `open` is undefined when none has.
 */
export function useBoardSheet(app: AppModel, selected: BotStore | null | undefined, online: BotStore[], size: { width: number; height: number }) {
  const [computer, setComputer] = useState<string | null>(null)
  const target = [selected, app.local, ...online].find((s) => s && online.includes(s) && hasBoard(s.hostVersion?.version))
  const store = computer ? app.store(computer) : null
  const sheet = (
    <Sheet open={computer !== null} onClose={() => setComputer(null)} width={Math.min(720, size.width - 80)} height={size.height} dismissOnDim>
      {store ? (
        <StoreContext.Provider value={store}>
          <BoardView key={computer} />
        </StoreContext.Provider>
      ) : null}
    </Sheet>
  )
  return { open: target ? () => setComputer(target.computer.id) : undefined, sheet }
}

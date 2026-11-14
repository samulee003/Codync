import { Switch } from '../../components/Controls'
import { Icon } from '../../components/Icon'
import { useTr } from '../../lib/use-language'
import type { ComputerAccess } from '../../store/computer-access'

/**
 * Settings → Computer access: run the bots' computer tools on agent-desktop, so several bots can work at once in the
 * background, instead of the default driver (one bot at a time). Hidden for a host that predates the choice.
 */
export function DriverSwitch({ access }: { access: ComputerAccess }) {
  const tr = useTr()
  const screen = access.screen
  if (!screen?.driver) return null
  const forced = screen.driverForced === true
  const on = forced || screen.driver === 'agentDesktop'
  const installed = screen.agentDesktopInstalled === true
  return (
    <div className="computer-access-driver">
      <Switch on={on} disabled={forced} label="Several bots at once" onChange={(next) => void access.setDriver(next ? 'agentDesktop' : 'default')}>
        <span className="computer-access-driver-text">
          <strong>{tr('Several bots at once')}</strong>
          <span>{tr('Bots use apps in the background with agent-desktop, so more than one can work at the same time.')}</span>
        </span>
      </Switch>
      {on && !installed ? (
        <p className="computer-access-driver-note" role="status">
          <Icon name="exclamationmark.triangle" size={12} />
          <span>{tr('agent-desktop isn’t installed, so bots still take turns. Install it with: npm install -g agent-desktop')}</span>
        </p>
      ) : null}
    </div>
  )
}

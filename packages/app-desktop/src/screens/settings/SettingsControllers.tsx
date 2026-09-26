import { ControllerMappings } from '../../components/ControllerMappings'
import { KeyboardBindings } from '../../components/KeyboardBindings'

export function SettingsControllers() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: 32 }}>
      <ControllerMappings />
      <KeyboardBindings />
    </div>
  )
}

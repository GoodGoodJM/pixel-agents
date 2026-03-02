import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export interface Platform {
  postMessage(msg: unknown): void
  onMessage(handler: (msg: unknown) => void): () => void
}

function routeMessage(msg: unknown): void {
  const m = msg as Record<string, unknown>
  const type = m.type as string
  switch (type) {
    case 'webviewReady':
      invoke('webview_ready')
      break
    case 'focusAgent':
      invoke('focus_agent', { id: m.id })
      break
    case 'saveLayout':
      invoke('save_layout', { layout: m.layout })
      break
    case 'saveAgentSeats':
      invoke('save_agent_seats', { seats: m.seats })
      break
    case 'setSoundEnabled':
      invoke('set_sound_enabled', { enabled: m.enabled })
      break
    case 'exportLayout':
      invoke('export_layout')
      break
    case 'importLayout':
      invoke('import_layout')
      break
  }
}

function subscribeMessages(handler: (msg: unknown) => void): () => void {
  const eventNames = [
    'agentCreated', 'agentClosed',
    'agentToolStart', 'agentToolDone', 'agentToolsClear',
    'agentStatus', 'agentToolPermission', 'agentToolPermissionClear',
    'subagentToolStart', 'subagentToolDone', 'subagentClear',
    'subagentToolPermission',
    'existingAgents', 'layoutLoaded', 'settingsLoaded',
  ]
  const unlisteners: Array<() => void> = []
  for (const name of eventNames) {
    listen(name, (event) => {
      handler(event.payload)
    }).then((u) => unlisteners.push(u))
  }
  return () => unlisteners.forEach((u) => u())
}

export const platform: Platform = {
  postMessage: routeMessage,
  onMessage: subscribeMessages,
}

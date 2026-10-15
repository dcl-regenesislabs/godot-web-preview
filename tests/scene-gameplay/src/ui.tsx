import { engine, Transform, MeshRenderer, Material, AvatarEmoteCommand, executeTask } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'
import { onSceneReadyObservable, onPlayerExpressionObservable } from '@dcl/sdk/observables'
import ReactEcs, { ReactEcsRenderer, UiEntity, Label } from '@dcl/sdk/react-ecs'
import { movePlayerTo, triggerEmote, triggerSceneEmote, stopEmote, teleportTo, changeRealm } from '~system/RestrictedActions'
import { getWorldTime, getExplorerInformation } from '~system/Runtime'

let status = 'Choose an action'
let position = '', camera = '', avatar = '', expression = '', emoteState = '', locale = '', time = 0, ready = 0
const buttons: [string, () => Promise<unknown>][] = [
  ['Move A', () => movePlayerTo({ newRelativePosition: { x: 4, y: 0, z: 4 }, cameraTarget: { x: 4, y: 1, z: 12 }, avatarTarget: { x: 12, y: 0, z: 4 } })],
  ['Move B', () => movePlayerTo({ newRelativePosition: { x: 12, y: 0, z: 12 }, cameraTarget: { x: 4, y: 1, z: 12 }, avatarTarget: { x: 12, y: 0, z: 4 } })],
  ['Outside (blocked)', () => movePlayerTo({ newRelativePosition: { x: 40, y: 0, z: 40 } })],
  ['Wave', () => triggerEmote({ predefinedEmote: 'wave' })],
  ['Sit (loop)', () => triggerSceneEmote({ src: 'assets/sit.glb', loop: true })],
  ['Stop emote', () => stopEmote({})],
  ['Teleport prompt', () => teleportTo({ worldCoordinates: { x: 0, y: 0 } })],
  ['Realm prompt', () => changeRealm({ realm: 'genesis-plaza.dcl.eth', message: 'Gameplay test: cancel this prompt' })]
]

function run(label: string, action: () => Promise<unknown>) {
  console.log(`GAMEPLAY request=${label}`)
  executeTask(async () => {
    try { await action(); status = `${label}: request accepted` }
    catch (error) { status = `${label}: ${String(error)}` }
    console.log(`GAMEPLAY result=${status}`)
  })
}

export function main() {
  onSceneReadyObservable.add(() => { ready++; console.log(`GAMEPLAY ready=${ready}`) })
  onPlayerExpressionObservable.add(event => {
    expression = event.expressionId.includes(':scene-emote:') ? 'custom scene emote' : event.expressionId.split(':').pop() ?? ''
    console.log(`GAMEPLAY expression=${event.expressionId}`)
  })
  AvatarEmoteCommand.onChange(engine.PlayerEntity, command => {
    emoteState = String(command?.state)
    console.log(`GAMEPLAY emote-state=${emoteState}`)
  })
  executeTask(async () => { locale = (await getExplorerInformation({})).configurations.locale })
  let elapsed = 0
  engine.addSystem(dt => {
    const player = Transform.getOrNull(engine.PlayerEntity), view = Transform.getOrNull(engine.CameraEntity)
    if (player) {
      const p = player.position, q = player.rotation
      position = `${p.x.toFixed(2)}, ${p.y.toFixed(2)}, ${p.z.toFixed(2)}`
      avatar = `${q.x.toFixed(2)}, ${q.y.toFixed(2)}, ${q.z.toFixed(2)}, ${q.w.toFixed(2)}`
    }
    if (view) { const q = view.rotation; camera = `${q.x.toFixed(2)}, ${q.y.toFixed(2)}, ${q.z.toFixed(2)}, ${q.w.toFixed(2)}` }
    elapsed += dt
    if (elapsed >= 2) {
      elapsed = 0
      executeTask(async () => { time = (await getWorldTime({})).seconds })
      console.log(`GAMEPLAY position=${position} camera=${camera} avatar=${avatar} time=${time}`)
    }
  })
  for (const x of [4, 12]) {
    const marker = engine.addEntity()
    Transform.create(marker, { position: { x, y: 0.05, z: x }, scale: { x: 2, y: 0.1, z: 2 } })
    MeshRenderer.setBox(marker)
    Material.setPbrMaterial(marker, { albedoColor: x === 4 ? Color4.Red() : Color4.Blue() })
  }
  ReactEcsRenderer.setUiRenderer(() => (
    <UiEntity uiTransform={{ width: '100%', height: '100%' }}>
      <UiEntity uiTransform={{ positionType: 'absolute', position: { left: '30%', top: 20 }, width: 760, height: 360, flexDirection: 'column' }} uiBackground={{ color: Color4.create(0.08, 0.1, 0.2, 0.95) }}>
        <UiEntity uiTransform={{ width: '100%', height: 160, flexWrap: 'wrap' }}>
          {buttons.map(([label, action]) => <UiEntity key={label} uiTransform={{ width: 240, height: 45, margin: 4 }} uiBackground={{ color: Color4.create(0.15, 0.3, 0.75, 1) }} onMouseDown={() => run(label, action)}>
            <Label value={label} fontSize={22} uiTransform={{ width: '100%', height: '100%' }} />
          </UiEntity>)}
        </UiEntity>
        <Label value={`${status}\nPosition: ${position}\nCamera: ${camera}\nAvatar: ${avatar}\nEmote: ${expression} / state ${emoteState}\nReady: ${ready} / time: ${time.toFixed(0)} / locale: ${locale}`} fontSize={19} uiTransform={{ width: '100%', height: 195 }} />
      </UiEntity>
    </UiEntity>
  ))
}

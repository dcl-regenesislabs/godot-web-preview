import { engine, Schemas, Transform, MeshRenderer, Material, PlayerIdentityData } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'
import { isServer, registerMessages, syncEntity } from '@dcl/sdk/network'
import ReactEcs, { ReactEcsRenderer, UiEntity, Label } from '@dcl/sdk/react-ecs'

const room = registerMessages({
  ping: Schemas.Map({ sequence: Schemas.Int }),
  pong: Schemas.Map({ sequence: Schemas.Int, total: Schemas.Int, found: Schemas.Boolean, x: Schemas.Float, z: Schemas.Float, near: Schemas.Boolean })
})
const Receipt = engine.defineComponent('test::ServerReceipt', { total: Schemas.Int })
let sent = 0
let replies = 0
let last = 0
let synced = 0
let status = 'Waiting for scene server'
let presence = 'Server has not checked your position'
let position = ''
let near = false

export function main() {
  if (isServer()) {
    const receipt = engine.addEntity()
    Receipt.create(receipt, { total: 0 })
    syncEntity(receipt, [Receipt.componentId])
    room.onMessage('ping', (data, context) => {
      if (!context) return
      const total = ++Receipt.getMutable(receipt).total
      console.log(`SCENE_COMMS_SERVER sequence=${data.sequence} total=${total} sender=${context.from}`)
      let found = false, x = 0, z = 0
      for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData, Transform)) {
        if (identity.address.toLowerCase() !== context.from.toLowerCase()) continue
        const point = Transform.get(entity).position
        found = true
        x = point.x
        z = point.z
      }
      const near = found && Math.hypot(x - 8, z - 12) < 2
      console.log(`PULSE_SERVER sender=${context.from} found=${found} x=${x.toFixed(2)} z=${z.toFixed(2)} near=${near}`)
      void room.send('pong', { sequence: data.sequence, total, found, x, z, near }, { to: [context.from] })
    })
    console.log('SCENE_COMMS_SERVER ready')
    return
  }

  room.onReady(ready => { status = ready ? 'Connected to scene server' : 'Waiting for scene server' })
  room.onMessage('pong', (data) => {
    replies++
    last = data.sequence
    presence = data.found ? 'Server sees your player' : 'Server cannot see your player'
    position = data.found ? `Server position: ${data.x.toFixed(2)}, ${data.z.toFixed(2)}` : ''
    near = data.near
    console.log(`SCENE_COMMS_CLIENT sequence=${last} replies=${replies} total=${data.total}`)
  })
  engine.addSystem(() => {
    for (const [, receipt] of engine.getEntitiesWith(Receipt)) synced = receipt.total
  })
  const marker = engine.addEntity()
  Transform.create(marker, { position: { x: 8, y: 1, z: 12 } })
  MeshRenderer.setBox(marker)
  Material.setPbrMaterial(marker, { albedoColor: Color4.Red() })

  ReactEcsRenderer.setUiRenderer(() => (
    <UiEntity uiTransform={{ width: '100%', height: '100%' }}>
      <UiEntity
        uiTransform={{ positionType: 'absolute', position: { left: '30%', top: 120 }, width: 640, height: 280 }}
        uiBackground={{ color: Color4.create(0.15, 0.25, 0.65, 1) }}
        onMouseDown={() => {
          const sequence = ++sent
          console.log(`SCENE_COMMS_CLIENT send=${sequence}`)
          void room.send('ping', { sequence }).catch(() => { status = 'Send failed' })
        }}
      >
        <Label value={`${status}\nClick to ping server\nSent ${sent} / replies ${replies} / last ${last}\nSynced server count ${synced}\n${presence}\n${position} / near red cube: ${near}`} fontSize={24} uiTransform={{ width: '100%', height: '100%' }} />
      </UiEntity>
    </UiEntity>
  ))
}

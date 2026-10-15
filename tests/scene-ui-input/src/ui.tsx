import { engine, Transform, MeshRenderer, Material } from '@dcl/sdk/ecs'
import { Color4 } from '@dcl/sdk/math'
import ReactEcs, { ReactEcsRenderer, UiEntity, Label } from '@dcl/sdk/react-ecs'

let downs = 0
let ups = 0

export function main() {
  const marker = engine.addEntity()
  Transform.create(marker, { position: { x: 8, y: 1, z: 10 } })
  MeshRenderer.setBox(marker)
  Material.setPbrMaterial(marker, { albedoColor: Color4.Red() })

  ReactEcsRenderer.setUiRenderer(() => (
    <UiEntity uiTransform={{ width: '100%', height: '100%' }}>
      <UiEntity
        uiTransform={{ positionType: 'absolute', position: { left: '35%', top: 120 }, width: 480, height: 120 }}
        uiBackground={{ color: Color4.create(0.15, 0.25, 0.65, 1) }}
        onMouseDown={() => { downs++; console.log(`UI_INPUT down=${downs} up=${ups}`) }}
        onMouseUp={() => { ups++; console.log(`UI_INPUT down=${downs} up=${ups}`) }}
      >
        <Label value={`Scene button: down ${downs} / up ${ups}`} fontSize={28} uiTransform={{ width: '100%', height: '100%' }} />
      </UiEntity>
    </UiEntity>
  ))
}

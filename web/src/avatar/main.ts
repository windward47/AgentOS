// Haru — autoUpdate:true + PIXI ticker AFTER model update → our params visible at render

import * as PIXI from 'pixi.js';
import { Live2DModel } from 'pixi-live2d-display';
import 'pixi-live2d-display/cubism4';

const MODEL = '/live2d/models/haru/haru.model3.json';

let app: PIXI.Application, model: Live2DModel | null = null;
let mouthOpen = 0, currentScale = 0.16;
let eyeTargetX = 0, eyeTargetY = 0, eyeIdleAt = 0;

let invokeFn: any = null, gcwFn: any = null;
import('@tauri-apps/api/core').then(m => invokeFn = m.invoke).catch(() => {});
import('@tauri-apps/api/window').then(m => { gcwFn = m.getCurrentWindow;
  document.getElementById('drag-bar')?.addEventListener('mousedown', e => { if (e.button === 0) gcwFn()?.startDragging(); });
}).catch(() => {});
document.addEventListener('contextmenu', e => { e.preventDefault(); const c = document.getElementById('ctx-menu')!; c.style.left = Math.min(e.clientX, innerWidth - 110) + 'px'; c.style.top = Math.min(e.clientY, innerHeight - 50) + 'px'; c.style.display = 'block'; });
document.addEventListener('click', () => document.getElementById('ctx-menu')!.style.display = 'none');
(window as any).closeWindow = () => gcwFn?.()?.close();

(function lipTick() {
  if (invokeFn) invokeFn('get_lip_level').then((l: any) => { mouthOpen = Math.min(+l * 1.8, 1); }).catch(() => {});
  requestAnimationFrame(lipTick);
})();

document.addEventListener('pointermove', (e) => { eyeTargetX = (e.clientX / innerWidth) * 2 - 1; eyeTargetY = (e.clientY / innerHeight) * 2 - 1; eyeIdleAt = Date.now() + 3000; });
document.addEventListener('wheel', (e) => { e.preventDefault(); currentScale += e.deltaY > 0 ? -0.015 : 0.015; currentScale = Math.max(0.04, Math.min(0.40, currentScale)); if (model) model.scale.set(currentScale); }, { passive: false });

const dbg = document.createElement('div');
dbg.style.cssText = 'position:fixed;bottom:4px;left:4px;color:#0f0;font-size:9px;font-family:monospace;pointer-events:none;z-index:9999';
document.body.appendChild(dbg);

async function init() {
  app = new PIXI.Application({ width: innerWidth, height: innerHeight, backgroundAlpha: 0, antialias: true, resolution: devicePixelRatio || 1, autoDensity: true });
  document.getElementById('root')!.appendChild(app.view as HTMLCanvasElement);

  Live2DModel.registerTicker(PIXI.Ticker);
  model = await Live2DModel.from(MODEL, { autoUpdate: true, autoInteract: false });
  model.anchor.set(0.5, 0.5); model.x = app.renderer.width / 2; model.y = app.renderer.height / 2;
  model.scale.set(currentScale);
  app.stage.addChild(model as any);

  // ★ Register AFTER Live2DModel's own ticker callback → our params are the LAST thing before render
  app.ticker.add(() => {
    if (!model) return;
    const b = (model as any).internalModel?.coreModel;
    if (!b) return;

    const blinkT = Date.now() % 4000 / 4000;
    const eyeOpen = blinkT > 0.95 ? 0.05 : 1.0;
    const idle = Date.now() > eyeIdleAt;

    b.setParameterValueById?.('ParamMouthOpenY', mouthOpen);
    b.setParameterValueById?.('ParamEyeLOpen', eyeOpen);
    b.setParameterValueById?.('ParamEyeROpen', eyeOpen);
    b.setParameterValueById?.('ParamAngleX', idle ? 0 : eyeTargetX * 30);
    b.setParameterValueById?.('ParamAngleY', idle ? 0 : eyeTargetY * 30);

    dbg.textContent = `eye:${eyeTargetX.toFixed(2)},${eyeTargetY.toFixed(2)} idle:${idle} mouth:${mouthOpen.toFixed(2)}`;
  });

  console.log('[Haru] Ready ✓');
}

init();

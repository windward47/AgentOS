// Haru — PixiJS + pixi-live2d-display with transparent background
// Pattern from old avatar-agent: PixiJS Application handles FBO for transparency

import * as PIXI from 'pixi.js';
import { Live2DModel } from 'pixi-live2d-display';
import 'pixi-live2d-display/cubism4';

const MODEL = '/live2d/models/haru/haru.model3.json';

// ── State ──
let app: PIXI.Application;
let model: Live2DModel | null = null;
let mouthOpen = 0;
let eyeOpen = 1;
let eyeTargetX = 0, eyeTargetY = 0;
let eyeIdleAt = 0;

// ── Tauri ──
let invokeFn: any = null, gcwFn: any = null;
import('@tauri-apps/api/core').then(m => invokeFn = m.invoke).catch(() => {});
import('@tauri-apps/api/window').then(m => {
  gcwFn = m.getCurrentWindow;
  document.getElementById('drag-bar')?.addEventListener('mousedown', e => {
    if (e.button === 0) gcwFn()?.startDragging();
  });
}).catch(() => {});
document.addEventListener('contextmenu', e => {
  e.preventDefault();
  const c = document.getElementById('ctx-menu')!;
  c.style.left = Math.min(e.clientX, innerWidth - 110) + 'px';
  c.style.top = Math.min(e.clientY, innerHeight - 50) + 'px';
  c.style.display = 'block';
});
document.addEventListener('click', () => document.getElementById('ctx-menu')!.style.display = 'none');
(window as any).closeWindow = () => gcwFn?.()?.close();

// ── Lip-sync poll ──
(function lipTick() {
  if (invokeFn) invokeFn('get_lip_level').then((l: any) => {
    mouthOpen = Math.min(+l * 1.8, 1.0);
  }).catch(() => {});
  requestAnimationFrame(lipTick);
})();

// ── Eye tracking ──
document.addEventListener('mousemove', (e) => {
  eyeTargetX = (e.clientX / innerWidth) * 2 - 1;
  eyeTargetY = (e.clientY / innerHeight) * 2 - 1;
  eyeIdleAt = Date.now() + 3000;
});

// ── Init ──
async function init() {
  // ★ PixiJS 7.x constructor — backgroundAlpha: 0 = transparent ★
  app = new PIXI.Application({
    width: innerWidth,
    height: innerHeight,
    backgroundAlpha: 0,
    antialias: true,
    resolution: devicePixelRatio || 1,
    autoDensity: true,
  });

  const canvas = app.view as HTMLCanvasElement;
  canvas.style.width = '100%';
  canvas.style.height = '100%';
  document.getElementById('root')!.appendChild(canvas);

  Live2DModel.registerTicker(PIXI.Ticker);

  try {
    model = await Live2DModel.from(MODEL, { autoUpdate: true, autoInteract: false });
    model.anchor.set(0.5, 0.5);
    model.x = app.renderer.width / 2;
    model.y = app.renderer.height / 2;
    model.scale.set(0.18);
    app.stage.addChild(model as any);
    console.log('[Haru] Model loaded ✓');
    (window as any).__live2d = {
      setParam: (n: string, v: number) => {
        try { (model as any)?.internalModel?.coreModel?.setParameterValueById?.(n, v); } catch {}
      }
    };
  } catch (e) {
    console.error('[Haru] Model load failed:', e);
    return;
  }

  new ResizeObserver(() => {
    app.renderer.resize(innerWidth, innerHeight);
    if (model) { model.x = innerWidth / 2; model.y = innerHeight / 2; }
  }).observe(canvas);

  app.ticker.add(() => {
    if (!model) return;
    const cm = (model as any).internalModel?.coreModel;
    if (!cm) return;

    const blinkT = Date.now() % 4000 / 4000;
    eyeOpen = blinkT > 0.95 ? 0.05 : 1.0;

    const idle = Date.now() > eyeIdleAt;
    const ax = (idle ? 0 : eyeTargetX) * 30;
    const ay = (idle ? 0 : eyeTargetY) * 30;

    cm.setParameterValueById?.('ParamMouthOpenY', mouthOpen);
    cm.setParameterValueById?.('ParamEyeLOpen', eyeOpen);
    cm.setParameterValueById?.('ParamEyeROpen', eyeOpen);
    cm.setParameterValueById?.('ParamAngleX', ax);
    cm.setParameterValueById?.('ParamAngleY', ay);
  });
}

init();

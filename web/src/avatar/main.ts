// Haru Live2D renderer — Cubism 5 Core + pixi-live2d-display Cubism4 wrapper
import * as PIXI from 'pixi.js';
import { Live2DModel } from 'pixi-live2d-display';
import 'pixi-live2d-display/cubism4';

// Tauri API (lazy)
let invokeFn: any = null;
let gcwFn: any = null;
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
document.addEventListener('click', () => { document.getElementById('ctx-menu')!.style.display = 'none'; });
(window as any).closeWindow = () => gcwFn?.()?.close();

// State
let app: PIXI.Application;
let model: any = null;
let lipSmooth = 0;
const root = document.getElementById('root')!;

function status(msg: string) {
  root.innerHTML = `<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#ccc;font-size:12px;font-family:monospace;text-align:center;padding:20px;white-space:pre-wrap;">${msg}</div>`;
}

async function init() {
  if (!(window as any).Live2DCubismCore) { status('CubismCore not loaded'); return; }
  status('Starting…');

  // Create PixiJS app
  try {
    app = new PIXI.Application({
      width: innerWidth, height: innerHeight,
      backgroundAlpha: 0, antialias: true,
      resolution: devicePixelRatio || 1, autoDensity: true,
      view: document.createElement('canvas'),
    });
    (app.view as HTMLCanvasElement).style.display = 'block';
    root.innerHTML = '';
    root.appendChild(app.view as HTMLCanvasElement);
  } catch (e: any) { status('PixiJS: ' + String(e)); return; }

  // Register ticker for pixi-live2d-display
  Live2DModel.registerTicker(PIXI.Ticker);
  status('Loading Haru…');

  try {
    model = await Live2DModel.from('/live2d/models/haru/haru.model3.json', {
      autoUpdate: true, autoHitTest: false,
    });
    model.x = app.view.width / 2;
    model.y = app.view.height * 0.58;
    model.scale.set(0.18);
    app.stage.addChild(model);
    root.querySelector('div')?.remove();

    (window as any).__live2d = {
      setParam: (name: string, value: number) => {
        try { model?.internalModel?.coreModel?.setParamFloat(name, value); } catch {}
      }
    };

    // Idle motion (every 25s)
    async function idleLoop() {
      if (!model) return;
      try {
        const r = await fetch('/live2d/models/haru/motion/haru_g_idle.motion3.json');
        if (r.ok) {
          const buf = await r.arrayBuffer();
          model.internalModel.motionManager?.startMotion(
            model.internalModel.motionManager.createMotion(buf, 'idle'), false, 1);
        }
      } catch {}
    }
    idleLoop();
    setInterval(idleLoop, 25000);

    // Expression cycle (every 5-12s)
    (function exprLoop() {
      if (!model) { setTimeout(exprLoop, 5000); return; }
      const i = Math.floor(Math.random() * 26) + 1;
      const name = 'haru_g_m' + String(i).padStart(2, '0') + '.motion3.json';
      fetch('/live2d/models/haru/motion/' + name).then(r => {
        if (r.ok) r.arrayBuffer().then(buf => {
          const mm = model?.internalModel?.motionManager;
          if (mm) mm.startMotion(mm.createMotion(buf, name), false, 3);
        });
      }).catch(() => {});
      setTimeout(exprLoop, 5000 + Math.random() * 7000);
    })();

    // Eye blink
    let blinkTime = 0, blinkVal = 0;
    app.ticker.add(() => {
      if (Date.now() - blinkTime > 3000 + Math.random() * 3000) { blinkTime = Date.now(); blinkVal = 1; }
      if (blinkVal > 0.01) {
        blinkVal *= 0.85;
        const c = model?.internalModel?.coreModel;
        if (c) { try { c.setParamFloat('ParamEyeLOpen', 1 - blinkVal * 0.9); } catch {} }
      }
    });

    status(''); // Clear status
    console.log('[Haru] Model loaded');
  } catch (e: any) {
    status('Model error:\n' + String(e));
    console.error('[Haru]', e);
  }

  // Resize
  addEventListener('resize', () => {
    app.renderer.resize(innerWidth, innerHeight);
    if (model) { model.x = app.view.width / 2; model.y = app.view.height * 0.58; }
  });
}

// Lip-sync poll
(function lipTick() {
  if (invokeFn && model) {
    invokeFn('get_lip_level').then((l: any) => {
      const t = Math.min(Number(l) * 1.8, 1.0);
      lipSmooth += (t - lipSmooth) * (t > lipSmooth ? 0.3 : 0.12);
      try { model?.internalModel?.coreModel?.setParamFloat('ParamMouthOpenY', lipSmooth); } catch {}
    }).catch(() => {});
  }
  requestAnimationFrame(lipTick);
})();

init();

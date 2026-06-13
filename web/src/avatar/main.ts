import * as PIXI from 'pixi.js';
import { Live2DModel } from 'pixi-live2d-display';
import 'pixi-live2d-display/cubism4';

const MODEL = '/live2d/models/haru/haru.model3.json';
let app: PIXI.Application, model: Live2DModel | null = null;
let currentModelPath = 'haru/haru.model3.json'; // track active model
let mouthOpen = 0, currentScale = 0.12;
const DEF_SCALE = 0.12;
const DEF_X = 0.35, DEF_Y = 0.35; // default position (window fraction)
let eyeTargetX = 0, eyeTargetY = 0, eyeIdleAt = 0;
let voiceState: string = 'idle';
let frameN = 0;
// Listening pose: head tilt (ParamAngleX) + wider eyes
const LISTEN_ANGLE = 50;      // head turn
const LISTEN_BROW = 1.0;     // eyebrow raise (positive = up on this Haru)
let curAngle = 0, tgtAngle = 0;
let curBrow = 0.0, tgtBrow = 0.0;

let invokeFn: any = null, gcwFn: any = null;
import('@tauri-apps/api/core').then(m => invokeFn = m.invoke).catch(() => {});
import('@tauri-apps/api/window').then(m => {
  gcwFn = m.getCurrentWindow;
  document.getElementById('drag-bar')?.addEventListener('mousedown', e => { if (e.button === 0) gcwFn()?.startDragging(); });
}).catch(() => {});
// Listen for "switch model" event from Settings page
import('@tauri-apps/api/event').then(m => {
  m.listen('reset_model_position', () => {
    currentScale = DEF_SCALE;
    if (model) { model.x = app.renderer.width * DEF_X; model.y = app.renderer.height * DEF_Y; model.scale.set(currentScale); }
  });
  m.listen('switch_live2d_model', (event: any) => {
    const path = event.payload as string;
    if (path && path !== currentModelPath) loadModel(path);
  });
}).catch(() => {});
document.addEventListener('contextmenu', e => { e.preventDefault(); const c = document.getElementById('ctx-menu')!; c.style.left = Math.min(e.clientX, innerWidth - 110) + 'px'; c.style.top = Math.min(e.clientY, innerHeight - 50) + 'px'; c.style.display = 'block'; });
document.addEventListener('click', () => document.getElementById('ctx-menu')!.style.display = 'none');
(window as any).closeWindow = () => gcwFn?.()?.close();
let isPinned = true; // default from tauri.conf.json: alwaysOnTop=true
(window as any).togglePin = () => {
  isPinned = !isPinned;
  if (gcwFn) {
    const w = gcwFn();
    w?.setAlwaysOnTop(isPinned).catch(() => {});
  }
};

// Poll at ~10fps
(function avatarTick() {
  if (invokeFn && ++frameN % 6 === 0) {
    invokeFn('get_lip_level').then((l: any) => { mouthOpen = Math.min(+l * 1.8, 1); }).catch(() => {});
    invokeFn('get_voice_state').then((s: string) => {
      if (s !== voiceState) {
        voiceState = s;
        if (s === 'listening') { tgtAngle = LISTEN_ANGLE; tgtBrow = LISTEN_BROW; }
        else if (s === 'speaking') { tgtAngle = 0; tgtBrow = 0.0; }
        else { tgtAngle = 0; tgtBrow = 0.0; }
      }
    }).catch(() => {});
  }
  requestAnimationFrame(avatarTick);
})();

document.addEventListener('wheel', (e) => { e.preventDefault(); currentScale += e.deltaY > 0 ? -0.02 : 0.02; currentScale = Math.max(0.06, Math.min(0.50, currentScale)); if (model) model.scale.set(currentScale); }, { passive: false });

// Middle-mouse drag to reposition the model within the window
let dragModel = false, dragMx = 0, dragMy = 0, dragOx = 0, dragOy = 0;
document.addEventListener('mousedown', (e) => {
  if (e.button === 1) { e.preventDefault(); dragModel = true; dragMx = e.clientX; dragMy = e.clientY; dragOx = model?.x ?? 0; dragOy = model?.y ?? 0; }
});
document.addEventListener('mousemove', (e) => {
  if (dragModel && model) { model.x = dragOx + (e.clientX - dragMx); model.y = dragOy + (e.clientY - dragMy); }
});
document.addEventListener('mouseup', (e) => {
  if (e.button === 1) { dragModel = false; }
});
// Double-click to reset position + scale
document.addEventListener('dblclick', () => {
  currentScale = DEF_SCALE;
  if (model) { model.x = app.renderer.width * DEF_X; model.y = app.renderer.height * DEF_Y; model.scale.set(currentScale); }
});

async function loadModel(path: string) {
  if (model) { app.stage.removeChild(model as any); model = null; }
  const url = '/live2d/models/' + path;
  console.log('[Haru] Loading model:', url);
  try {
    const m = await Live2DModel.from(url, { autoUpdate: true, autoInteract: false });
    m.anchor.set(0.5, 0.5);
    m.x = app.renderer.width * 0.34; m.y = app.renderer.height * 0.34;
    m.scale.set(currentScale);
    app.stage.addChild(m as any);
    model = m;
    currentModelPath = path;
  } catch (err: any) {
    console.error('[Haru] Failed to load model:', url, err.message);
    // Fall back to haru if loading fails
    if (path !== 'haru/haru.model3.json') {
      console.log('[Haru] Falling back to default model');
      await loadModel('haru/haru.model3.json');
    }
  }
}

async function init() {
  app = new PIXI.Application({ width: innerWidth, height: innerHeight, backgroundAlpha: 0, antialias: true, resolution: devicePixelRatio || 1, autoDensity: true });
  app.ticker.maxFPS = 30;
  document.getElementById('root')!.appendChild(app.view as HTMLCanvasElement);
  Live2DModel.registerTicker(PIXI.Ticker);

  // Load saved model from config (wait for invoke to be available)
  if (invokeFn) {
    try {
      const cfg = await invokeFn('get_config') as { live2d_model?: string };
      if (cfg?.live2d_model) currentModelPath = cfg.live2d_model;
    } catch {}
  }
  await loadModel(currentModelPath);

  let hooked = false;
  app.ticker.add(() => {
    if (!model) return;
    const b = (model as any).internalModel?.coreModel;
    if (!b) return;

    // Cursor tracking at full 60fps for smooth head movement
    if (invokeFn) {
      invokeFn('get_cursor_pos').then(([cx, cy]: [number, number]) => {
        // Compute cursor offset relative to avatar window center
        // so head tracking works consistently regardless of window position
        const win = gcwFn?.();
        if (win) {
          Promise.all([win.outerPosition(), win.outerSize()]).then(([pos, size]) => {
            const winCx = pos.x + size.width / 2;
            const winCy = pos.y + size.height / 2;
            eyeTargetX = ((cx - winCx) / (size.width / 2));
            eyeTargetY = ((cy - winCy) / (size.height / 2));
          }).catch(() => {});
        } else {
          // Fallback: absolute screen (old behavior)
          eyeTargetX = (cx / screen.width) * 2 - 1;
          eyeTargetY = (cy / screen.height) * 2 - 1;
        }
        if (cx !== 0 || cy !== 0) eyeIdleAt = Date.now() + 3000;
      }).catch(() => {});
    }

    curAngle += (tgtAngle - curAngle) * 0.15;
    curBrow += (tgtBrow - curBrow) * 0.15;

    if (!hooked) {
      try {
      const orig = b.saveParameters.bind(b);
      b.saveParameters = () => {
        try {
        const blinkT = Date.now() % 4000 / 4000;
        const eyeOpen = blinkT > 0.95 ? 0.05 : 1.0;
        const idle = Date.now() > eyeIdleAt;
        b.setParameterValueById('ParamMouthOpenY', mouthOpen, 1);
        b.setParameterValueById('ParamEyeLOpen', eyeOpen, 1);
        b.setParameterValueById('ParamEyeROpen', eyeOpen, 1);
        b.setParameterValueById('ParamBrowLY', curBrow, 1);
        b.setParameterValueById('ParamBrowRY', curBrow, 1);
        b.setParameterValueById('ParamAngleX', (idle ? 0 : eyeTargetX * 30) + curAngle, 1);
        b.setParameterValueById('ParamAngleY', idle ? 0 : -eyeTargetY * 30, 1);
        if (voiceState === 'listening') {
          b.setParameterValueById('ParamAngleZ', 10, 1);
        }
        orig();
        } catch (e) { /* model may not have these params */ }
      };
      hooked = true;
      console.log('[Haru] hooked ✓');
      } catch (e) { console.warn('[Haru] hook failed:', e); }
    }
  });

  console.log('[Haru] Ready ✓');
}
init();

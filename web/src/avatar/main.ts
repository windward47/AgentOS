import * as PIXI from 'pixi.js';
import { Live2DModel } from 'pixi-live2d-display';
import 'pixi-live2d-display/cubism4';

const MODEL = '/live2d/models/haru/haru.model3.json';
let app: PIXI.Application, model: Live2DModel | null = null;
let mouthOpen = 0, currentScale = 0.16;
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

document.addEventListener('wheel', (e) => { e.preventDefault(); currentScale += e.deltaY > 0 ? -0.015 : 0.015; currentScale = Math.max(0.04, Math.min(0.40, currentScale)); if (model) model.scale.set(currentScale); }, { passive: false });

async function init() {
  app = new PIXI.Application({ width: innerWidth, height: innerHeight, backgroundAlpha: 0, antialias: true, resolution: devicePixelRatio || 1, autoDensity: true });
  document.getElementById('root')!.appendChild(app.view as HTMLCanvasElement);
  Live2DModel.registerTicker(PIXI.Ticker);
  model = await Live2DModel.from(MODEL, { autoUpdate: true, autoInteract: false });
  model.anchor.set(0.5, 0.5); model.x = app.renderer.width / 2; model.y = app.renderer.height / 2;
  model.scale.set(currentScale); app.stage.addChild(model as any);

  let hooked = false;
  app.ticker.add(() => {
    if (!model) return;
    const b = (model as any).internalModel?.coreModel;
    if (!b) return;

    // Cursor tracking at full 60fps for smooth head movement
    if (invokeFn) {
      invokeFn('get_cursor_pos').then(([cx, cy]: [number, number]) => {
        eyeTargetX = (cx / screen.width) * 2 - 1;
        eyeTargetY = (cy / screen.height) * 2 - 1;
        if (cx !== 0 || cy !== 0) eyeIdleAt = Date.now() + 3000;
      }).catch(() => {});
    }

    curAngle += (tgtAngle - curAngle) * 0.15;
    curBrow += (tgtBrow - curBrow) * 0.15;

    if (!hooked) {
      const orig = b.saveParameters.bind(b);
      b.saveParameters = () => {
        const blinkT = Date.now() % 4000 / 4000;
        const eyeOpen = blinkT > 0.95 ? 0.05 : 1.0;
        const idle = Date.now() > eyeIdleAt;
        b.setParameterValueById('ParamMouthOpenY', mouthOpen, 1);
        b.setParameterValueById('ParamEyeLOpen', eyeOpen, 1);
        b.setParameterValueById('ParamEyeROpen', eyeOpen, 1);
        // Listening: raise eyebrows (more visible than eye deformation)
        b.setParameterValueById('ParamBrowLY', curBrow, 1);
        b.setParameterValueById('ParamBrowRY', curBrow, 1);
        // head turn: mouse tracking + listening bias
        // Listening: big head turn + apply expression
        b.setParameterValueById('ParamAngleX', (idle ? 0 : eyeTargetX * 30) + curAngle, 1);
        b.setParameterValueById('ParamAngleY', idle ? 0 : -eyeTargetY * 30, 1);
        if (voiceState === 'listening') {
          b.setParameterValueById('ParamAngleZ', 10, 1); // slight tilt
        }
        orig();
      };
      hooked = true;
      console.log('[Haru] hooked ✓');
    }
  });

  console.log('[Haru] Ready ✓');
}
init();

// Haru Live2D Avatar — Cubism 5 SDK + Tauri lip-sync
// Full rendering pipeline: model, shaders, physics, expressions, lip-sync

import { CubismUserModel } from '../live2d/model/cubismusermodel';
import { CubismModel } from '../live2d/model/cubismmodel';
import { CubismFramework, Option, LogLevel } from '../live2d/live2dcubismframework';
import { CubismDefaultParameterId } from '../live2d/cubismdefaultparameterid';
import { CubismModelSettingJson } from '../live2d/cubismmodelsettingjson';
import { ICubismModelSetting } from '../live2d/icubismmodelsetting';
import { CubismMotion } from '../live2d/motion/cubismmotion';
import { CubismEyeBlink } from '../live2d/effect/cubismeyeblink';
import { CubismBreath, BreathParameterData } from '../live2d/effect/cubismbreath';
import { CubismMatrix44 } from '../live2d/math/cubismmatrix44';
import { CubismIdHandle } from '../live2d/id/cubismid';
import { CubismRenderer_WebGL } from '../live2d/rendering/cubismrenderer_webgl';

// Tauri
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
document.addEventListener('click', () => document.getElementById('ctx-menu')!.style.display = 'none');
(window as any).closeWindow = () => gcwFn?.()?.close();

const root = document.getElementById('root')!;
const BASE = '/live2d/models/haru/';
let canvas: HTMLCanvasElement, gl: WebGLRenderingContext;
let model: CubismUserModel, renderer: CubismRenderer_WebGL;
let cm: CubismModel; // convenience ref
let eyeBlink: CubismEyeBlink, breath: CubismBreath;
let idMouth: CubismIdHandle, idEyeL: CubismIdHandle;
let lipSmooth = 0;
let lastTime = 0;
const expressionNames: string[] = [];

function status(msg: string) {
  root.innerHTML = `<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#eee;font-size:12px;font-family:monospace;text-align:center;padding:20px;white-space:pre-wrap;">${msg}</div>`;
}

async function fetchBuf(path: string) { const r = await fetch(BASE + path); if (!r.ok) throw Error(`HTTP ${r.status}: ${path}`); return r.arrayBuffer(); }

async function init() {
  if (!(window as any).Live2DCubismCore) { status('CubismCore not loaded'); return; }
  status('Cubism 5 Framework…');

  CubismFramework.startUp(new Option());
  CubismFramework.initialize();

  canvas = document.createElement('canvas'); canvas.width = innerWidth; canvas.height = innerHeight; canvas.style.display = 'block';
  root.innerHTML = ''; root.appendChild(canvas);
  gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true })!
    || canvas.getContext('webgl', { alpha: true, premultipliedAlpha: true })!;
  if (!gl) { status('WebGL not available'); return; }
  console.log('[Haru] GL:', gl instanceof WebGL2RenderingContext ? 'WebGL2' : 'WebGL1');

  // Model3.json
  status('Loading model3.json…');
  const settingJson = await fetchBuf('haru.model3.json');
  const modelSetting = new CubismModelSettingJson(settingJson, settingJson.byteLength) as ICubismModelSetting;

  // Moc3
  status('Loading .moc3…');
  model = new CubismUserModel();
  model.loadModel(await fetchBuf('haru.moc3'));
  cm = model.getModel()!;
  if (!cm) { status('Model creation failed'); return; }

  // Physics + pose (optional)
  try { model.loadPhysics(await fetchBuf('haru.physics3.json'), 0); } catch {}
  try { model.loadPose(await fetchBuf('haru.pose3.json'), 0); } catch {}

  // Eye blink
  eyeBlink = CubismEyeBlink.create(modelSetting);

  // Breath
  breath = CubismBreath.create();
  const idMgr = CubismFramework.getIdManager();
  breath.setParameters([
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamAngleX), 0, 15, 6.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamAngleY), 0, 8, 3.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamAngleZ), 0, 10, 5.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamBodyAngleX), 0, 4, 15.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamBreath), 0.5, 0.5, 3.2345, 1),
  ]);

  // Param IDs
  idMouth = idMgr.getId(CubismDefaultParameterId.ParamMouthOpenY);
  idEyeL = idMgr.getId(CubismDefaultParameterId.ParamEyeLOpen);

  // Renderer — Cubism 5 shader init requires two passes:
  // 1. First generateShaders() fires async fetch of .frag/.vert files
  // 2. After files load, _shaderSets must be cleared + regenerated
  //    (SDK bug: _isShaderLoaded never set, and first gen uses empty sources)
  status('Loading shaders…');
  model.createRenderer(canvas.width, canvas.height);
  renderer = model.getRenderer();
  renderer.startUp(gl);

  const { CubismShaderManager_WebGL } = await import('../live2d/rendering/cubismshader_webgl');
  const shader = CubismShaderManager_WebGL.getInstance().getShader(gl);
  shader.setShaderPath('/live2d/');
  shader.generateShaders(); // triggers async file fetches

  // Wait for async fetches to populate source strings
  for (let i = 0; i < 60; i++) {
    if ((shader as any)._fragShaderSrcPremultipliedAlpha) break;
    await new Promise(r => setTimeout(r, 250));
  }

  // Re-generate with populated sources
  (shader as any)._shaderSets = [];
  shader.generateShaders();

  // Mark loaded so doDrawModel() doesn't keep regenerating
  (shader as any)._isShaderLoaded = true;
  (shader as any)._isShaderLoading = false;

  // Idle motion
  status('Loading idle…');
  try {
    const mm = (model as any)._motionManager;
    const idleBuf = await fetchBuf('motion/haru_g_idle.motion3.json');
    const m = CubismMotion.create(idleBuf, idleBuf.byteLength);
    if (m) {
      m.setEffectIds([], []); // eye blink + lip sync handled separately
      mm.startMotionPriority(m, false, 1);
    }
  } catch {}

  // Expression list
  try {
    const chk = await fetch(BASE + 'motion/haru_g_m01.motion3.json');
    if (chk.ok) for (let i = 1; i <= 26; i++) expressionNames.push('haru_g_m' + String(i).padStart(2, '0') + '.motion3.json');
  } catch {}
  setTimeout(cycleExpr, 5000);

  status('');
  console.log('[Haru] Ready. Drawables:', cm.getDrawableCount());
  lastTime = performance.now();
  requestAnimationFrame(loop);
}

async function cycleExpr() {
  if (!cm || expressionNames.length === 0) { setTimeout(cycleExpr, 5000); return; }
  const name = expressionNames[Math.floor(Math.random() * expressionNames.length)];
  try {
    const buf = await (await fetch(BASE + 'motion/' + name)).arrayBuffer();
    const m = CubismMotion.create(buf, buf.byteLength);
    if (m) {
      m.setEffectIds([], []);
      (model as any)._motionManager?.startMotionPriority(m, false, 3);
    }
  } catch {}
  setTimeout(cycleExpr, 5000 + Math.random() * 7000);
}

function loop() {
  const now = performance.now();
  const dt = Math.min((now - lastTime) / 1000, 0.1);
  lastTime = now;
  if (!cm || !renderer) { requestAnimationFrame(loop); return; }

  // Motion updates (wrapped — errors in motions shouldn't kill the renderer)
  try {
    const mm = (model as any)._motionManager;
    mm?.updateMotion?.(cm, dt);
    (model as any)._expressionManager?.updateMotion?.(cm, dt);
  } catch (e) { /* motion parse error — model still draws */ }

  // Effects
  eyeBlink?.updateParameters(cm, dt);
  breath?.updateParameters(cm, dt);
  (model as any)._physics?.update?.(cm, dt);
  (model as any)._pose?.update?.(cm, dt);

  // Lip-sync
  const mi = cm.getParameterIndex(idMouth);
  if (mi >= 0) cm.addParameterValueById(mi, lipSmooth);

  // Apply + draw
  cm.update();
  cm.loadParameters();

  gl.clearColor(0.0, 0.0, 0.0, 0.0);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

  const fbo = gl.getParameter(gl.FRAMEBUFFER_BINDING);
  renderer.setRenderState(fbo, [0, 0, canvas.width, canvas.height]);

  const matrix = new CubismMatrix44();
  matrix.scale(0.18, 0.18);
  matrix.translateRelative(canvas.width / 2 / 0.18, canvas.height * 0.55 / 0.18);
  renderer.setMvpMatrix(matrix);
  renderer.drawModel();

  if (canvas.width !== innerWidth || canvas.height !== innerHeight) {
    canvas.width = innerWidth; canvas.height = innerHeight;
  }

  requestAnimationFrame(loop);
}

(function lipTick() {
  if (invokeFn && cm) {
    invokeFn('get_lip_level').then((l: any) => {
      lipSmooth += (Math.min(+l * 1.8, 1) - lipSmooth) * 0.25;
    }).catch(() => {});
  }
  requestAnimationFrame(lipTick);
})();

init().catch(e => { status('Error: ' + String(e)); console.error(e); });

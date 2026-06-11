// Haru Live2D Avatar — Cubism 5 SDK · follows official demo pattern
// Cuts out all the LApp* abstractions.  Direct init sequence:
//   CubismFramework → CubismUserModel → loadModel → loadTextures →
//   createRenderer + startUp + loadShaders → bindTextures → loop

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
import { CubismShaderManager_WebGL } from '../live2d/rendering/cubismshader_webgl';

// Tauri
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

// ── State ──
const root = document.getElementById('root')!;
const BASE = '/live2d/models/haru/';
let canvas: HTMLCanvasElement, gl: WebGLRenderingContext;
let model: CubismUserModel, cm: CubismModel, renderer: CubismRenderer_WebGL;
let eyeBlink: CubismEyeBlink, breath: CubismBreath;
let idMouth: CubismIdHandle;
let lipSmooth = 0, lastTime = 0;
const expressionNames: string[] = [];

function status(msg: string) {
  root.innerHTML = `<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#ccc;font-size:12px;font-family:monospace;text-align:center;padding:20px;white-space:pre-wrap;">${msg}</div>`;
}

async function loadTexture(path: string): Promise<WebGLTexture> {
  const img = await new Promise<HTMLImageElement>((resolve, reject) => {
    const i = new Image(); i.crossOrigin = 'anonymous';
    i.onload = () => resolve(i); i.onerror = () => reject(new Error(path));
    i.src = BASE + path;
  });
  const tex = gl.createTexture()!;
  gl.bindTexture(gl.TEXTURE_2D, tex);
  gl.pixelStorei(gl.UNPACK_PREMULTIPLY_ALPHA_WEBGL, 1);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.generateMipmap(gl.TEXTURE_2D);
  gl.bindTexture(gl.TEXTURE_2D, null);
  return tex;
}

async function init() {
  if (!(window as any).Live2DCubismCore) { status('CubismCore not loaded'); return; }
  status('Cubism 5…');

  // 1. Framework
  CubismFramework.startUp(new Option());
  CubismFramework.initialize();

  // 2. Canvas + WebGL
  canvas = document.createElement('canvas'); canvas.width = innerWidth; canvas.height = innerHeight;
  canvas.style.display = 'block'; root.innerHTML = ''; root.appendChild(canvas);
  gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true })!
    || canvas.getContext('webgl', { alpha: true, premultipliedAlpha: true })!;
  if (!gl) { status('WebGL not available'); return; }
  console.log('[Haru] GL:', gl instanceof WebGL2RenderingContext ? 'WebGL2' : 'WebGL1');
  gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

  // 3. Model setting
  status('Loading model3.json…');
  const sjson = await (await fetch(BASE + 'haru.model3.json')).arrayBuffer();
  const setting = new CubismModelSettingJson(sjson, sjson.byteLength) as ICubismModelSetting;

  // 4. CubismUserModel + moc3
  status('Loading .moc3…');
  model = new CubismUserModel();
  const mocBuf = await (await fetch(BASE + 'haru.moc3')).arrayBuffer();
  model.loadModel(mocBuf);  // calls CubismMoc.create + .createModel() internally
  cm = model.getModel()!;
  if (!cm) { status('Model creation failed'); return; }

  // 5. Optional: physics + pose
  try { model.loadPhysics(await (await fetch(BASE + 'haru.physics3.json')).arrayBuffer(), 0); } catch {}
  try { model.loadPose(await (await fetch(BASE + 'haru.pose3.json')).arrayBuffer(), 0); } catch {}

  // 6. Eye blink + breath
  eyeBlink = CubismEyeBlink.create(setting);
  breath = CubismBreath.create();
  const idMgr = CubismFramework.getIdManager();
  breath.setParameters([
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamAngleX), 0, 15, 6.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamAngleY), 0, 8, 3.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamAngleZ), 0, 10, 5.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamBodyAngleX), 0, 4, 15.5345, 0.5),
    new BreathParameterData(idMgr.getId(CubismDefaultParameterId.ParamBreath), 0.5, 0.5, 3.2345, 1),
  ]);
  idMouth = idMgr.getId(CubismDefaultParameterId.ParamMouthOpenY);

  // 7. Renderer — create + startUp + loadShaders
  status('Loading shaders…');
  model.createRenderer(canvas.width, canvas.height);
  renderer = model.getRenderer();
  renderer.startUp(gl);
  renderer.setIsPremultipliedAlpha(true);

  // Two-pass shader init (see previous commit for rationale)
  const shader = CubismShaderManager_WebGL.getInstance().getShader(gl);
  shader.setShaderPath('/live2d/');
  shader.generateShaders();
  for (let i = 0; i < 60; i++) {
    if ((shader as any)._fragShaderSrcPremultipliedAlpha) break;
    await new Promise(r => setTimeout(r, 250));
  }
  (shader as any)._shaderSets = [];
  shader.generateShaders();
  (shader as any)._isShaderLoaded = true;
  (shader as any)._isShaderLoading = false;

  // 8.  ★★★ LOAD + BIND TEXTURES (the missing piece) ★★★
  status('Loading textures…');
  const texCount = setting.getTextureCount();
  if (texCount > 0) {
    for (let i = 0; i < texCount; i++) {
      const path = setting.getTextureFileName(i);
      if (!path) continue;
      const tex = await loadTexture(path);
      renderer.bindTexture(i, tex);  // ← THIS is what makes the model visible!
      console.log('[Haru] Texture', i, path);
    }
  }

  // 9. Idle motion
  try {
    const mm = (model as any)._motionManager;
    const buf = await (await fetch(BASE + 'motion/haru_g_idle.motion3.json')).arrayBuffer();
    const m = CubismMotion.create(buf, buf.byteLength);
    if (m) { m.setEffectIds([], []); mm.startMotionPriority(m, false, 1); }
  } catch {}

  // 10. Expression list
  try {
    const chk = await fetch(BASE + 'motion/haru_g_m01.motion3.json');
    if (chk.ok) for (let i = 1; i <= 26; i++) expressionNames.push('haru_g_m' + String(i).padStart(2, '0') + '.motion3.json');
  } catch {}
  setTimeout(cycleExpr, 5000);

  status('');
  console.log('[Haru] Ready. Drawables:', cm.getDrawableCount(), 'Textures:', texCount);
  lastTime = performance.now();
  requestAnimationFrame(loop);
}

async function cycleExpr() {
  if (!cm || expressionNames.length === 0) { setTimeout(cycleExpr, 5000); return; }
  const name = expressionNames[Math.floor(Math.random() * expressionNames.length)];
  try {
    const buf = await (await fetch(BASE + 'motion/' + name)).arrayBuffer();
    const m = CubismMotion.create(buf, buf.byteLength);
    if (m) { m.setEffectIds([], []); (model as any)._motionManager?.startMotionPriority(m, false, 3); }
  } catch {}
  setTimeout(cycleExpr, 5000 + Math.random() * 7000);
}

function loop() {
  const now = performance.now();
  const dt = Math.min((now - lastTime) / 1000, 0.1);
  lastTime = now;
  if (!cm || !renderer) { requestAnimationFrame(loop); return; }

  const mm = (model as any)._motionManager;
  try { mm?.updateMotion?.(cm, dt); (model as any)._expressionManager?.updateMotion?.(cm, dt); } catch {}
  eyeBlink?.updateParameters(cm, dt);
  breath?.updateParameters(cm, dt);
  try { (model as any)._physics?.update?.(cm, dt); (model as any)._pose?.update?.(cm, dt); } catch {}

  const mi = cm.getParameterIndex(idMouth);
  if (mi >= 0) cm.addParameterValueById(mi, lipSmooth);

  cm.update(); cm.loadParameters();

  // Clear + set state + draw (matches official demo LAppView.render + LAppSubdelegate.update)
  // Clear + draw — matches official LAppSubdelegate.update() + LAppLive2DManager.onUpdate()
  gl.clearColor(0.102, 0.102, 0.180, 1.0);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.enable(gl.DEPTH_TEST); gl.depthFunc(gl.LEQUAL); gl.clearDepth(1.0);

  const projection = new CubismMatrix44();
  const modelMatrix = model.getModelMatrix();

  // Step 1: Scale model to Cubism view coords ([-1,1])
  //   Haru's canvas width is ~3000+, setWidth(2.0) scales it to 2 units wide
  modelMatrix.setWidth(2.0);

  // Step 2: Map [-1,1] view coords to canvas pixels
  if (canvas.width < canvas.height) {
    projection.scale(1.0, canvas.width / canvas.height);
  } else {
    projection.scale(canvas.height / canvas.width, 1.0);
  }

  // Step 3: Combine model matrix into projection
  projection.multiplyByMatrix(modelMatrix);

  // Step 4: Set MVP + draw (matches LAppModel.draw())
  renderer.setMvpMatrix(projection);
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

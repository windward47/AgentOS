// Haru Avatar — Official Cubism 5 Demo pattern, verbatim init sequence
// Stripped of multi-model, sprites, touch. Lip-sync bolted on after init.
//
// Init sequence (mirrors LAppDelegate + LAppSubdelegate + LAppModel):
// ① CubismFramework.startUp + initialize
// ② canvas.getContext('webgl2')
// ③ fetch model3.json → CubismModelSettingJson
// ④ fetch .moc3 → CubismMoc.create → loadModel
// ⑤ fetch physics → loadPhysics  (optional)
// ⑥ fetch pose → loadPose          (optional)
// ⑦ CubismEyeBlink.create + CubismBreath
// ⑧ fetch each motion → CubismMotion.create → preloadGroup
// ⑨ When motions done: createRenderer → startUp(gl) → loadShaders
// ⑩ fetch each texture → createTexture → renderer.bindTexture
// ⑪ setPremultipliedAlpha(true)
// ⑫ Render loop: update params → model.draw(projection)

import { CubismFramework, Option, LogLevel } from '../live2d/live2dcubismframework';
import { CubismModelSettingJson } from '../live2d/cubismmodelsettingjson';
import { CubismUserModel } from '../live2d/model/cubismusermodel';
import { CubismEyeBlink } from '../live2d/effect/cubismeyeblink';
import { CubismBreath, BreathParameterData } from '../live2d/effect/cubismbreath';
import { CubismDefaultParameterId } from '../live2d/cubismdefaultparameterid';
import { CubismMotion } from '../live2d/motion/cubismmotion';
import { CubismMatrix44 } from '../live2d/math/cubismmatrix44';
import { CubismShaderManager_WebGL } from '../live2d/rendering/cubismshader_webgl';

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

// ── Config ──
const RES = '/live2d/demo/Haru/';  // model3.json, .moc3, textures, motions live here
const SHADER = '/live2d/';          // .frag/.vert shader files
const MOTION_GROUP = 'Idle';
const EXPRESSION_COUNT = 26;        // haru_g_m01 .. m26

// ── State ──
let canvas: HTMLCanvasElement, gl: WebGLRenderingContext;
let model: CubismUserModel;
let renderer: any;
let eyeBlink: CubismEyeBlink, breath: CubismBreath;
let idMouth: any, idEyeL: any;
let allMotions: Map<string, CubismMotion> = new Map();
let motionCount = 0, totalMotionCount = 0;
let initialized = false;
let lipSmooth = 0, lastTime = 0;
const root = document.getElementById('root')!;

function status(msg: string) {
  root.innerHTML = `<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#ccc;font-size:12px;font-family:monospace;text-align:center;padding:20px;white-space:pre-wrap;">${msg}</div>`;
}

// ── ① Framework ──
function initFramework() {
  status('① CubismFramework…');
  CubismFramework.startUp(new Option());
  CubismFramework.initialize();
}

// ── ② Canvas ──
function initCanvas() {
  status('② WebGL…');
  canvas = document.createElement('canvas');
  canvas.width = innerWidth; canvas.height = innerHeight;
  canvas.style.display = 'block';
  root.innerHTML = '';
  root.appendChild(canvas);
  gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true })!;
  if (!gl) { status('WebGL2 not available'); throw new Error('WebGL2'); }
  console.log('[Haru] WebGL2 ready');
}

// ── ③-⑨ Full init (mirrors LAppModel.loadAssets → setupModel) ──
async function loadModel() {
  status('③ Loading model3.json…');
  const sjson = await (await fetch(RES + 'Haru.model3.json')).arrayBuffer();
  const setting = new CubismModelSettingJson(sjson, sjson.byteLength);
  console.log('[Haru] model3.json loaded');

  status('④ Loading .moc3…');
  model = new CubismUserModel();
  const mocBuf = await (await fetch(RES + 'Haru.moc3')).arrayBuffer();
  model.loadModel(mocBuf);
  if (!model.getModel()) { throw new Error('Moc3 creation failed'); }
  console.log('[Haru] .moc3 loaded, drawables:', model.getModel()!.getDrawableCount());

  // ⑤-⑥ Physics + Pose
  try { status('⑤ Physics…'); model.loadPhysics(await (await fetch(RES + 'Haru.physics3.json')).arrayBuffer(), 0); } catch {}
  try { status('⑥ Pose…'); model.loadPose(await (await fetch(RES + 'Haru.pose3.json')).arrayBuffer(), 0); } catch {}
  console.log('[Haru] Physics/Pose loaded');

  // ⑦ Eye blink + Breath
  status('⑦ Blink + Breath…');
  eyeBlink = CubismEyeBlink.create(setting as any);
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
  idEyeL = idMgr.getId(CubismDefaultParameterId.ParamEyeLOpen);
  console.log('[Haru] Effects ready');

  // ⑧ Motions
  status('⑧ Loading motions…');
  const motionCountTotal = (setting as any).getMotionCount?.(MOTION_GROUP) ?? 0;
  totalMotionCount = motionCountTotal;
  for (let i = 0; i < motionCountTotal; i++) {
    const name = `${MOTION_GROUP}_${i}`;
    const file = (setting as any).getMotionFileName?.(MOTION_GROUP, i);
    if (!file) continue;
    const buf = await (await fetch(RES + file)).arrayBuffer();
    const motion = CubismMotion.create(buf, buf.byteLength);
    if (motion) {
      motion.setEffectIds([], []);
      allMotions.set(name, motion);
      motionCount++;
    }
  }
  console.log('[Haru] Motions:', motionCount, '/', totalMotionCount);

  // ⑨ Renderer + shaders + textures
  status('⑨ Renderer + shaders…');
  model.createRenderer(canvas.width, canvas.height);
  renderer = model.getRenderer();
  renderer.startUp(gl);
  renderer.setIsPremultipliedAlpha(true);

  // Wait for shader async load
  const shader = CubismShaderManager_WebGL.getInstance().getShader(gl);
  shader.setShaderPath(SHADER);
  shader.generateShaders();
  for (let i = 0; i < 120; i++) {
    if ((shader as any)._isShaderLoaded) break;
    await new Promise(r => setTimeout(r, 250));
  }
  if (!(shader as any)._isShaderLoaded) { throw new Error('Shader load timeout'); }
  console.log('[Haru] Shaders loaded');

  // ⑩ Textures
  status('⑩ Textures…');
  const texCount = (setting as any).getTextureCount?.() ?? 0;
  for (let i = 0; i < texCount; i++) {
    const path = (setting as any).getTextureFileName?.(i);
    if (!path) continue;
    const tex = await loadTexture(path);
    renderer.bindTexture(i, tex);
    console.log('[Haru] Texture', i, path);
  }
  console.log('[Haru] Textures:', texCount);

  initialized = true;
  status('');
  console.log('[Haru] ✓ INIT COMPLETE. Starting render loop.');
  lastTime = performance.now();
  requestAnimationFrame(loop);
}

// ── Texture loader ──
async function loadTexture(path: string): Promise<WebGLTexture> {
  const img = await new Promise<HTMLImageElement>((resolve, reject) => {
    const i = new Image(); i.crossOrigin = 'anonymous';
    i.onload = () => resolve(i); i.onerror = () => reject(new Error(path));
    i.src = RES + path;
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

// ── Render loop (mirrors LAppSubdelegate.update + LAppLive2DManager.onUpdate) ──
function loop() {
  const now = performance.now();
  const dt = Math.min((now - lastTime) / 1000, 0.1);
  lastTime = now;

  if (!initialized || !model || !renderer) { requestAnimationFrame(loop); return; }

  const cm = model.getModel()!;

  // Update motions (mirrors CubismUserModel internal update)
  const mm = (model as any)._motionManager;
  const em = (model as any)._expressionManager;
  try { mm?.updateMotion?.(cm, dt); } catch {}
  try { em?.updateMotion?.(cm, dt); } catch {}

  // Effects
  eyeBlink?.updateParameters(cm, dt);
  breath?.updateParameters(cm, dt);
  try { (model as any)._physics?.update?.(cm, dt); } catch {}
  try { (model as any)._pose?.update?.(cm, dt); } catch {}

  // Lip-sync
  const mi = cm.getParameterIndex(idMouth);
  if (mi >= 0) cm.addParameterValueById(mi, lipSmooth);

  cm.update();
  cm.loadParameters();

  // Clear + draw — matches LAppSubdelegate.update()
  gl.clearColor(0.102, 0.102, 0.180, 1.0);
  gl.clear(gl.COLOR_BUFFER_BIT);
  gl.enable(gl.DEPTH_TEST);
  gl.depthFunc(gl.LEQUAL);
  gl.clearDepth(1.0);

  // Projection — matches LAppLive2DManager.onUpdate()
  const projection = new CubismMatrix44();
  const modelMatrix = model.getModelMatrix();
  modelMatrix.setWidth(2.0);
  if (canvas.width < canvas.height) {
    projection.scale(1.0, canvas.width / canvas.height);
  } else {
    projection.scale(canvas.height / canvas.width, 1.0);
  }
  projection.multiplyByMatrix(modelMatrix);

  // draw — matches LAppModel.draw()
  renderer!.setMvpMatrix(projection);
  renderer!.drawModel();

  // Resize
  if (canvas.width !== innerWidth || canvas.height !== innerHeight) {
    canvas.width = innerWidth;
    canvas.height = innerHeight;
  }

  requestAnimationFrame(loop);
}

// ── Lip-sync poll ──
(function lipTick() {
  if (invokeFn && initialized) {
    invokeFn('get_lip_level').then((l: any) => {
      lipSmooth += (Math.min(+l * 1.8, 1) - lipSmooth) * 0.25;
    }).catch(() => {});
  }
  requestAnimationFrame(lipTick);
})();

// ── Idle motion cycler ──
setInterval(() => {
  if (!model || allMotions.size === 0) return;
  const keys = [...allMotions.keys()];
  const name = keys[Math.floor(Math.random() * keys.length)];
  const motion = allMotions.get(name);
  if (motion) {
    const mm = (model as any)._motionManager;
    mm?.startMotionPriority?.(motion, false, 1);
  }
}, 8000 + Math.random() * 5000);

// ── Entry ──
(async () => {
  try {
    initFramework();
    initCanvas();
    await loadModel();
  } catch (e: any) {
    status('FATAL: ' + String(e));
    console.error(e);
  }
})();

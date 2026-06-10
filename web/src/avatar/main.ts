// Haru WebGL Renderer — Cubism 5 Core WASM (no framework, no pixi-live2d-display)
// Direct Live2DCubismCore API: load .moc3 → create model → set params → update + draw

const Live2D = (window as any).Live2DCubismCore;
if (!Live2D) throw new Error('Live2DCubismCore not loaded');

// Tauri API
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

const root = document.getElementById('root')!;
const BASE = '/live2d/models/haru/';

// ── Globals ──
let gl: WebGLRenderingContext;
let canvas: HTMLCanvasElement;
let model: any;        // Live2DCubismCore Model
let moc: any;          // Live2DCubismCore Moc
let textures: WebGLTexture[] = [];
let paramIds: Record<string, number> = {};
let mX = 0, mY = 0, mScale = 0.18;
let lipSmooth = 0;
let blinkTime = 0, blinkVal = 0;
let breathTime = 0;

function status(msg: string) {
  root.innerHTML = `<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#fff;font-size:12px;font-family:monospace;text-align:center;padding:20px;white-space:pre-wrap;">${msg}</div>`;
}

// ── WebGL helpers ──
function createTexture(img: HTMLImageElement): WebGLTexture {
  const t = gl.createTexture()!;
  gl.bindTexture(gl.TEXTURE_2D, t);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, img);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
  gl.generateMipmap(gl.TEXTURE_2D);
  return t;
}

// ── Load .moc3, textures, model ──
async function init() {
  status('Loading model…');

  // Canvas
  canvas = document.createElement('canvas');
  canvas.width = innerWidth; canvas.height = innerHeight;
  canvas.style.display = 'block';
  root.innerHTML = '';
  root.appendChild(canvas);
  gl = canvas.getContext('webgl', { alpha: true, premultipliedAlpha: true })!;
  if (!gl) { status('WebGL not available'); return; }
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);

  // Load .moc3
  const mocResp = await fetch(BASE + 'haru.moc3');
  if (!mocResp.ok) { status('moc3 not found'); return; }
  const mocBuf = await mocResp.arrayBuffer();
  moc = Live2D.Moc.fromArrayBuffer(mocBuf);
  if (!moc) { status('Moc creation failed'); return; }

  model = Live2D.Model.fromMoc(moc);
  if (!model) { status('Model creation failed'); return; }

  // Load model3.json for param names
  const settingResp = await fetch(BASE + 'haru.model3.json');
  const setting = await settingResp.json();

  // Collect param IDs
  if (setting.Parameters) {
    for (const p of setting.Parameters) {
      paramIds[p.Id] = model.getParameterIndex(p.Id);
    }
  }

  // Load textures
  const texPaths = setting.FileReferences?.Textures || [];
  for (const tp of texPaths) {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject();
      img.src = BASE + tp;
    });
    textures.push(createTexture(img));
  }

  status('');
  console.log('[Haru] Model ready. Textures:', textures.length, 'Params:', Object.keys(paramIds).length);
  (window as any).__live2d = {
    setParam(n: string, v: number) { setParam(n, v); }
  };
  requestAnimationFrame(loop);
}

// ── Render loop ──
function loop() {
  if (!model) return;
  const now = Date.now();
  const dt = 1/60;

  // Breathing
  breathTime += dt;
  setParam('ParamBreath', 0.5 + 0.5 * Math.sin(breathTime * 2 * Math.PI / 3.2));

  // Eye blink
  if (now - blinkTime > 3000 + Math.random() * 3000) { blinkTime = now; blinkVal = 1; }
  if (blinkVal > 0.01) { blinkVal *= 0.85; setParam('ParamEyeLOpen', 1 - blinkVal * 0.9); }

  // Lip-sync
  setParam('ParamMouthOpenY', lipSmooth);

  // Head angle — slight idle sway
  const sway = Math.sin(now * 0.001) * 2;
  setParam('ParamAngleX', sway);
  setParam('ParamBodyAngleX', sway * 0.5);

  // Update + draw
  model.update();
  model.setTexture(0, textures[0]); // bind textures before draw
  for (let i = 0; i < textures.length; i++) model.setTexture(i, textures[i]);

  // Setup view matrix
  const matrix = new Float32Array([
    mScale, 0, 0, 0,
    0, mScale, 0, 0,
    0, 0, 1, 0,
    mX * mScale, mY * mScale, 0, 1,
  ]);
  model.setMatrix(matrix);
  model.draw(matrix);

  gl.flush();

  canvas.width = innerWidth;
  canvas.height = innerHeight;
  gl.viewport(0, 0, innerWidth, innerHeight);
  mX = innerWidth / 2 / mScale;
  mY = innerHeight * 0.58 / mScale;

  requestAnimationFrame(loop);
}

function setParam(id: string, value: number) {
  if (!model) return;
  const idx = paramIds[id];
  if (idx !== undefined && idx >= 0) {
    model.setParameterValueById(idx, value);
  }
}

// Lip-sync poll
(function lipTick() {
  if (invokeFn && model) {
    invokeFn('get_lip_level').then((l: any) => {
      lipSmooth += (Math.min(+l * 1.8, 1) - lipSmooth) * 0.25;
    }).catch(() => {});
  }
  requestAnimationFrame(lipTick);
})();

init();

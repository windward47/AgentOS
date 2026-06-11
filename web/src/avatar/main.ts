// Haru — Official Cubism 5 Demo + transparency + lip-sync + eye tracking
// All SDK type issues worked around with `as any` — Cubism 5 TS declarations are incomplete

import * as ShaderData from '../live2d/shaders/shaders';
import { CubismShader_WebGL } from '../live2d/rendering/cubismshader_webgl';
import { CubismModel } from '../live2d/model/cubismmodel';
import { CubismFramework } from '../live2d/live2dcubismframework';

// ═══ Shader injection ═══
(CubismShader_WebGL.prototype as any).loadShaders = async function () {
  this._vertShaderSrc = ShaderData.vertshadersrc_vert;
  this._vertShaderSrcMasked = ShaderData.vertshadersrcmasked_vert;
  this._vertShaderSrcSetupMask = ShaderData.vertshadersrcsetupmask_vert;
  this._fragShaderSrcSetupMask = ShaderData.fragshadersrcsetupmask_frag;
  this._fragShaderSrcPremultipliedAlpha = ShaderData.fragshadersrcpremultipliedalpha_frag;
  this._fragShaderSrcMaskPremultipliedAlpha = ShaderData.fragshadersrcmaskpremultipliedalpha_frag;
  this._fragShaderSrcMaskInvertedPremultipliedAlpha = ShaderData.fragshadersrcmaskinvertedpremultipliedalpha_frag;
  this._vertShaderSrcCopy = ShaderData.vertshadersrccopy_vert;
  this._fragShaderSrcCopy = ShaderData.fragshadersrccopy_frag;
  this._fragShaderSrcColorBlend = ShaderData.fragshadersrccolorblend_frag;
  this._fragShaderSrcAlphaBlend = ShaderData.fragshadersrcalphablend_frag;
  this._vertShaderSrcBlend = ShaderData.vertshadersrcblend_vert;
  this._fragShaderSrcBlend = ShaderData.fragshadersrcpremultipliedalphablend_frag;
  this._isShaderLoading = false;
  this._isShaderLoaded = true;
};

// ═══ Lip-sync: saveParameters hook ═══
(window as any).__lipValue = 0;

let _mouthIdx: number = -1;
function getMouthIdx(model: any): number {
  if (_mouthIdx < 0) {
    try {
      const idMgr = (CubismFramework as any).getIdManager();
      const mouthId = idMgr.getId('ParamMouthOpenY');
      const idx = model.getParameterIndex(mouthId);
      if (idx >= 0) _mouthIdx = idx;
    } catch {}
  }
  return _mouthIdx;
}

const _origSave = (CubismModel.prototype as any).saveParameters;
(CubismModel.prototype as any).saveParameters = function () {
  const lip = (window as any).__lipValue;
  if (lip > 0.001) {
    try {
      const idx = getMouthIdx(this);
      if (idx >= 0) this.setParameterValueById(idx, lip);
    } catch {}
  }
  return _origSave.call(this);
};

// ═══ Import demo classes ═══
import { LAppDelegate } from './demo/lappdelegate';
import { LAppSubdelegate } from './demo/lappsubdelegate';

// ═══ Transparent clear ═══
const _origSubUpdate = (LAppSubdelegate.prototype as any).update;
(LAppSubdelegate.prototype as any).update = function () {
  try { this.getGl().clearColor(0, 0, 0, 0); } catch {}
  return _origSubUpdate.call(this);
};

// ═══ Tauri ═══
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

// ═══ Lip-sync poll ═══
(function lipTick() {
  if (invokeFn) {
    invokeFn('get_lip_level')
      .then((l: any) => { (window as any).__lipValue = Math.min(+l * 1.8, 1.0); })
      .catch(() => {});
  }
  requestAnimationFrame(lipTick);
})();

// ═══ Eye tracking ═══
let eyeIdleAt = 0;
let eyeRawX = 0, eyeRawY = 0;
const EYE_IDLE_MS = 3000;

document.addEventListener('mousemove', (e) => {
  eyeRawX = (e.clientX / innerWidth) * 2 - 1;
  eyeRawY = (e.clientY / innerHeight) * 2 - 1;
  eyeIdleAt = Date.now() + EYE_IDLE_MS;
});

(function eyeTick() {
  const idle = Date.now() > eyeIdleAt;
  const tx = idle ? 0 : eyeRawX;
  const ty = idle ? 0 : eyeRawY;
  (window as any).__eyeX += (tx - (window as any).__eyeX) * 0.08;
  (window as any).__eyeY += (ty - (window as any).__eyeY) * 0.08;
  try {
    const m = (LAppDelegate.getInstance() as any)
      ?._subdelegates?.[0]?._live2dManager?._models?.[0];
    if (m?.setDragging) {
      m.setDragging((window as any).__eyeX, (window as any).__eyeY);
    }
  } catch {}
  requestAnimationFrame(eyeTick);
})();

// ═══ Entry ═══
if (!(window as any).Live2DCubismCore) {
  document.getElementById('root')!.innerHTML = '<div style="color:red;padding:20px;">CubismCore not loaded</div>';
} else if (!LAppDelegate.getInstance().initialize()) {
  document.getElementById('root')!.innerHTML = '<div style="color:red;padding:20px;">Init failed</div>';
} else {
  LAppDelegate.getInstance().run();
}

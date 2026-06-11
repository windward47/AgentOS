// Haru — Cubism 5 Demo + lip-sync + eye tracking
// Transparency: not supported on Windows WebView2 + raw WebGL (PixiJS can,
// but we use CubismRenderer directly). Known limitation, accepted.

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

// ═══ Lip-sync (saveParameters hook) ═══
(window as any).__lipValue = 0;
let _mouthIdx: number = -1;
function getMouthIdx(model: any): number {
  if (_mouthIdx < 0) {
    try {
      _mouthIdx = model.getParameterIndex(
        (CubismFramework as any).getIdManager().getId('ParamMouthOpenY')
      );
    } catch {}
  }
  return _mouthIdx;
}
const _origSave = (CubismModel.prototype as any).saveParameters;
(CubismModel.prototype as any).saveParameters = function () {
  const lip = (window as any).__lipValue;
  if (lip > 0.001) {
    try { const i = getMouthIdx(this); if (i >= 0) this.setParameterValueById(i, lip); } catch {}
  }
  return _origSave.call(this);
};

// ═══ Demo ═══
import { LAppDelegate } from './demo/lappdelegate';
import { LAppView } from './demo/lappview';

// Neutralize demo touch handlers that crash on missing _gear sprite
(LAppView.prototype as any).onTouchesEnded = function () {};
(LAppView.prototype as any).onTouchesBegan = function () {};
(LAppView.prototype as any).onTouchesMoved = function () {};

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
  if (invokeFn) invokeFn('get_lip_level').then((l: any) => {
    (window as any).__lipValue = Math.min(+l * 1.8, 1.0);
  }).catch(() => {});
  requestAnimationFrame(lipTick);
})();

// ═══ Eye tracking: mousemove → setDragging, idle after 3s ═══
let eyeIdleAt = 0, eyeRawX = 0, eyeRawY = 0;
document.addEventListener('mousemove', (e) => {
  eyeRawX = (e.clientX / innerWidth) * 2 - 1;
  eyeRawY = (e.clientY / innerHeight) * 2 - 1;
  eyeIdleAt = Date.now() + 3000;
});
(function eyeTick() {
  const idle = Date.now() > eyeIdleAt;
  const tx = idle ? 0 : eyeRawX, ty = idle ? 0 : eyeRawY;
  (window as any).__eyeX += (tx - (window as any).__eyeX) * 0.08;
  (window as any).__eyeY += (ty - (window as any).__eyeY) * 0.08;
  try {
    const m = (LAppDelegate.getInstance() as any)?._subdelegates?.[0]?._live2dManager?._models?.[0];
    if (m?.setDragging) m.setDragging((window as any).__eyeX, (window as any).__eyeY);
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

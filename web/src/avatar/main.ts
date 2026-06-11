// Haru — Official Cubism 5 Demo
// Shaders embedded at import time (Tauri SPA fallback intercepts .frag/.vert fetch)

import * as ShaderData from '../live2d/shaders/shaders';
import { CubismShader_WebGL } from '../live2d/rendering/cubismshader_webgl';
import { LAppDelegate } from './demo/lappdelegate';

// Monkey-patch: replace private loadShaders with inline source injection
(CubismShader_WebGL.prototype as any).loadShaders = async function() {
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

(function lipTick() {
  if (invokeFn) {
    invokeFn('get_lip_level').then((l: any) => {
      try {
        const mgr = (LAppDelegate.getInstance() as any)._subdelegates?.[0]?._live2dManager;
        const m = mgr?._models?.[0];
        const cm = m?.getModel?.();
        if (cm) {
          const idx = cm.getParameterIndex?.('ParamMouthOpenY');
          if (idx >= 0) cm.setParameterValueById(idx, Math.min(+l * 1.8, 1));
        }
      } catch {}
    }).catch(() => {});
  }
  requestAnimationFrame(lipTick);
})();

if (!(window as any).Live2DCubismCore) {
  document.getElementById('root')!.innerHTML = '<div style="color:red;padding:20px;">CubismCore not loaded</div>';
} else if (!LAppDelegate.getInstance().initialize()) {
  document.getElementById('root')!.innerHTML = '<div style="color:red;padding:20px;">Init failed</div>';
} else {
  LAppDelegate.getInstance().run();
}

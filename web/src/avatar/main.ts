// Haru Avatar — Directly runs the official Cubism 5 TypeScript Demo
// All LApp* classes imported verbatim, only config/init adapted

import { LAppDelegate } from './demo/lappdelegate';
import * as LAppDefine from './demo/lappdefine';

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

// Lip-sync
let lipSmooth = 0;
(function lipTick() {
  if (invokeFn) {
    invokeFn('get_lip_level').then((l: any) => {
      lipSmooth += (Math.min(+l * 1.8, 1) - lipSmooth) * 0.25;
      // Drive mouth parameter on the Live2D model managed by LAppLive2DManager
      try {
        const mgr = (LAppDelegate.getInstance() as any)._subdelegates?.[0]?._live2dManager;
        const m = mgr?._models?.[0];
        const cm = m?.getModel?.();
        if (cm) {
          const idx = cm.getParameterIndex?.('ParamMouthOpenY');
          if (idx >= 0) cm.setParameterValueById(idx, lipSmooth);
        }
      } catch {}
    }).catch(() => {});
  }
  requestAnimationFrame(lipTick);
})();

// ── Init ──
if (!(window as any).Live2DCubismCore) {
  document.getElementById('root')!.innerHTML = '<div style="color:red;padding:20px;">CubismCore not loaded</div>';
} else {
  if (!LAppDelegate.getInstance().initialize()) {
    document.getElementById('root')!.innerHTML = '<div style="color:red;padding:20px;">Init failed</div>';
  } else {
    LAppDelegate.getInstance().run();
  }
}

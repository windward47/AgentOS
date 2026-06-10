// Avatar window — Tauri API + lip-sync for CSS character
let invokeFn: any = null, gcw: any = null;
import('@tauri-apps/api/core').then(m => invokeFn = m.invoke).catch(() => {});
import('@tauri-apps/api/window').then(m => {
  gcw = m.getCurrentWindow;
  document.getElementById('drag-bar')?.addEventListener('mousedown', e => {
    if (e.button === 0) gcw()?.startDragging();
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
(window as any).closeWindow = () => gcw?.()?.close();

// Expression states: [browDeg, mouthScale, mouthColor, blushOpacity]
const expressions = [
  { b: -3, mc: '#e0705c', bo: .3 },
  { b: 4, mc: '#f0a0a0', bo: .6 },
  { b: -8, mc: '#c06050', bo: .2 },
  { b: 6, mc: '#ff8080', bo: 0 },
  { b: 0, mc: '#e0705c', bo: .4 },
];
let exprIdx = 0;
function cycleExpr() {
  const e = expressions[exprIdx = (exprIdx + 1) % expressions.length];
  const bl = document.getElementById('browL'), br = document.getElementById('browR');
  const mouth = document.getElementById('mouth');
  if (bl) bl.style.transform = `rotate(${e.b}deg)`;
  if (br) br.style.transform = `rotate(${-e.b}deg)`;
  if (mouth) mouth.style.background = e.mc;
  document.querySelectorAll<HTMLElement>('.blush').forEach(b => { b.style.opacity = String(e.bo); });
}
setInterval(cycleExpr, 5000 + Math.random() * 7000);

let lipSmooth = 0;
function lipTick() {
  if (invokeFn) {
    invokeFn('get_lip_level').then((l: any) => {
      const t = Math.min(+l * 1.6, 1);
      lipSmooth += (t - lipSmooth) * (t > lipSmooth ? .35 : .15);
      const sy = 0.3 + lipSmooth * 2.8;
      const mouth = document.getElementById('mouth');
      const head = document.getElementById('head');
      const bangs = document.getElementById('bangs');
      if (mouth) { mouth.style.transform = `scaleY(${Math.min(sy, 3.5)})`; mouth.style.height = (8 + lipSmooth * 10) + 'px'; }
      if (head) head.style.transform = `scaleY(${1 - lipSmooth * 0.02})`;
      if (bangs) bangs.style.transform = `translateX(-50%) scaleY(${1 + lipSmooth * 0.03})`;
    }).catch(() => {});
  }
  requestAnimationFrame(lipTick);
}
lipTick();

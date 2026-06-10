// Live2D Avatar Renderer — Tauri avatar window entry
// PixiJS v7 + pixi-live2d-display + Cubism 4 + Haru model
// Idle animation · expression cycling · lip-sync · eye blink

import * as PIXI from 'pixi.js';
import { Live2DModel } from 'pixi-live2d-display';
import 'pixi-live2d-display/cubism4';

// ── Lazy Tauri API ──
let invokeFn: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;
let getCurrentWindowFn: (() => any) | null = null;

async function loadTauri() {
    try {
        const api = await import('@tauri-apps/api/core');
        invokeFn = api.invoke;
        const win = await import('@tauri-apps/api/window');
        getCurrentWindowFn = win.getCurrentWindow;
    } catch { /* browser */ }
}

// ── State ──
let app: PIXI.Application;
let model: Live2DModel | null = null;
const expressionNames: string[] = [];
let lipSmooth = 0;
let blinkTimer = 0;
let blinkValue = 0;

// ── Eye blink ──
function eyeBlink() {
    if (!model) return;
    const now = Date.now();
    if (now - blinkTimer > 3000 + Math.random() * 3000) { blinkTimer = now; blinkValue = 1; }
    if (blinkValue > 0.01) {
        blinkValue *= 0.85;
        const core = (model as any).internalModel?.coreModel;
        if (core) {
            try { core.setParamFloat('ParamEyeLOpen', 1 - blinkValue * 0.9); } catch {}
            try { core.setParamFloat('ParamEyeROpen', 1 - blinkValue * 0.9); } catch {}
        }
    }
}

// ── Lip-sync ──
function lipTick() {
    if (invokeFn) {
        invokeFn('get_lip_level').then((level) => {
            const target = Math.min((level as number) * 1.8, 1.0);
            const speed = target > lipSmooth ? 0.3 : 0.12;
            lipSmooth += (target - lipSmooth) * speed;
            const core = (model as any)?.internalModel?.coreModel;
            if (core) { try { core.setParamFloat('ParamMouthOpenY', lipSmooth); } catch {} }
        }).catch(() => {});
    }
    requestAnimationFrame(lipTick);
}

// ── Expression cycling ──
async function cycleExpression() {
    if (!model || expressionNames.length === 0) { setTimeout(cycleExpression, 5000); return; }
    const name = expressionNames[Math.floor(Math.random() * expressionNames.length)];
    try {
        const resp = await fetch(`/live2d/models/haru/motion/${name}`);
        if (!resp.ok) return;
        const buffer = await resp.arrayBuffer();
        const mm = (model as any).internalModel?.motionManager;
        if (mm) { const m = mm.createMotion(buffer, name); mm.startMotion(m, false, 3); }
    } catch {}
    setTimeout(cycleExpression, 5000 + Math.random() * 7000);
}

// ── Idle motion ──
async function playIdle() {
    if (!model) return;
    try {
        const resp = await fetch('/live2d/models/haru/motion/haru_g_idle.motion3.json');
        if (!resp.ok) return;
        const buffer = await resp.arrayBuffer();
        const mm = (model as any).internalModel?.motionManager;
        if (mm) { const m = mm.createMotion(buffer, 'idle'); mm.startMotion(m, false, 1); }
    } catch {}
}

// ── Drag ──
function setupDrag() {
    document.getElementById('drag-bar')?.addEventListener('mousedown', (e) => {
        if (e.button !== 0 || !getCurrentWindowFn) return;
        getCurrentWindowFn()?.startDragging();
    });
}

// ── Context menu ──
function setupContextMenu() {
    document.addEventListener('contextmenu', (e) => {
        e.preventDefault();
        const menu = document.getElementById('ctx-menu');
        if (!menu) return;
        menu.style.left = Math.min(e.clientX, innerWidth - 110) + 'px';
        menu.style.top = Math.min(e.clientY, innerHeight - 50) + 'px';
        menu.style.display = 'block';
    });
    document.addEventListener('click', () => {
        const el = document.getElementById('ctx-menu');
        if (el) el.style.display = 'none';
    });
    (window as any).closeWindow = () => getCurrentWindowFn?.()?.close();
}

// ── Init ──
async function init() {
    app = new PIXI.Application({
        width: innerWidth, height: innerHeight,
        backgroundAlpha: 0, antialias: true,
        resolution: devicePixelRatio || 1, autoDensity: true,
    });
    document.getElementById('root')!.appendChild(app.view as HTMLCanvasElement);

    Live2DModel.registerTicker(PIXI.Ticker);

    try {
        model = await Live2DModel.from('/live2d/models/haru/haru.model3.json', { autoUpdate: true, autoHitTest: false });
        model.x = app.view.width / 2;
        model.y = app.view.height * 0.58;
        model.scale.set(0.18);
        app.stage.addChild(model as any);

        (window as any).__live2d = {
            setParam: (name: string, value: number) => {
                try { (model as any)?.internalModel?.coreModel?.setParamFloat(name, value); } catch {}
            }
        };

        // Discover expression motions
        try {
            const chk = await fetch('/live2d/models/haru/motion/haru_g_m01.motion3.json');
            if (chk.ok) for (let i = 1; i <= 26; i++) expressionNames.push(`haru_g_m${String(i).padStart(2, '0')}.motion3.json`);
        } catch {}

        playIdle();
        setInterval(playIdle, 25000);
        setTimeout(cycleExpression, 5000);
    } catch (err) {
        console.error('Live2D load failed:', err);
        document.getElementById('root')!.innerHTML = '<div style="display:flex;align-items:center;justify-content:center;height:100%;color:#aaa;font-size:24px;">🎭</div>';
        return;
    }

    addEventListener('resize', () => {
        app.renderer.resize(innerWidth, innerHeight);
        if (model) { model.x = app.view.width / 2; model.y = app.view.height * 0.58; }
    });

    app.ticker.add(() => eyeBlink());
    lipTick();
}

loadTauri();
setupDrag();
setupContextMenu();
init();

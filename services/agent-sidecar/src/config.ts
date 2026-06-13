/**
 * Load omp configuration from ~/.omp/agent/models.yml
 * and build pi-catalog Model objects for custom providers.
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { load as parseYaml } from "js-yaml";
import { buildModel } from "@oh-my-pi/pi-catalog/build";
import type { Api, Model } from "@oh-my-pi/pi-ai";

const OMP_CONFIG_DIR = join(homedir(), ".omp", "agent");
const MODELS_YML = join(OMP_CONFIG_DIR, "models.yml");
const CONFIG_YML = join(OMP_CONFIG_DIR, "config.yml");
const COMPANION_CONFIG = join(homedir(), ".companion", "config.json");

// ── CompanionConfig types (mirrors Rust companion_core::config::CompanionConfig) ──

export interface CompanionConfig {
    sandbox_path: string;
    llm_provider: string;
    asr_provider: string;
    tts_provider: string;
    system_mode: boolean;
    tts_auto_play: boolean;
    vad_threshold: number;
    voice_mode: string;
    tts_voice: string;
    tts_speed: number;
    default_api_key: string;
    user_name: string;
    custom_system_prompt: string;
    live2d_model: string;
    llm: { provider: string; url: string | null; key: string | null; model: string | null };
    asr: { provider: string; url: string | null; key: string | null; model: string | null };
    tts: { provider: string; url: string | null; key: string | null; model: string | null };
    custom_providers: Array<{ provider: string; url: string | null; key: string | null; model: string | null }>;
    global_voice: {
        record_hotkey: string;
        tts_hotkey: string;
        inject_mode_switch_hotkey: string;
        engine_switch_hotkey: string;
        inject_mode: string;
        asr_engine: string;
        tts_engine: string;
    };
}

export function loadCompanionConfig(): CompanionConfig {
    try {
        if (!existsSync(COMPANION_CONFIG)) return defaultConfig();
        const raw = JSON.parse(readFileSync(COMPANION_CONFIG, "utf-8"));
        return { ...defaultConfig(), ...raw };
    } catch (err) {
        process.stderr.write(`[config] Failed to load ${COMPANION_CONFIG}: ${err}\n`);
        return defaultConfig();
    }
}

export function saveCompanionConfig(cfg: CompanionConfig): void {
    const dir = join(homedir(), ".companion");
    if (!existsSync(dir)) {
        mkdirSync(dir, { recursive: true });
    }
    writeFileSync(COMPANION_CONFIG, JSON.stringify(cfg, null, 2), "utf-8");
}

function defaultConfig(): CompanionConfig {
    return {
        sandbox_path: join(homedir(), ".companion", "sandbox"),
        llm_provider: "siliconflow",
        asr_provider: "xiaomi",
        tts_provider: "xiaomi",
        system_mode: false,
        tts_auto_play: false,
        vad_threshold: 0.3,
        voice_mode: "ptt",
        tts_voice: "茉莉",
        tts_speed: 1.0,
        default_api_key: "",
        user_name: "User",
        custom_system_prompt: "You are Companion, a helpful desktop AI assistant.",
        live2d_model: "haru",
        llm: { provider: "", url: null, key: null, model: null },
        asr: { provider: "", url: null, key: null, model: null },
        tts: { provider: "", url: null, key: null, model: null },
        custom_providers: [],
        global_voice: {
            record_hotkey: "Alt+`",
            tts_hotkey: "Alt+T",
            inject_mode_switch_hotkey: "Alt+Shift+V",
            engine_switch_hotkey: "Alt+Shift+E",
            inject_mode: "keyboard",
            asr_engine: "mimo",
            tts_engine: "mimo-tts",
        },
    };
}

interface RawProviderConfig {
    baseUrl?: string;
    apiKey?: string;
    api?: string;
    compat?: {
        supportsDeveloperRole?: boolean;
        supportsMultipleSystemMessages?: boolean;
    };
    models?: Array<{
        id: string;
        name?: string;
        input?: string[];
        contextWindow?: number;
        maxTokens?: number;
    }>;
}

interface RawModelsYaml {
    providers?: Record<string, RawProviderConfig>;
}

interface RawConfigYaml {
    modelRoles?: {
        default?: string;
        smol?: string;
        plan?: string;
    };
}

export interface LoadedModel {
    provider: string;
    providerConfig: RawProviderConfig;
    modelSpec: NonNullable<RawProviderConfig["models"]>[number];
}

function safeLoad<T>(path: string): T {
    try {
        if (!existsSync(path)) return {} as T;
        const text = readFileSync(path, "utf-8");
        return (parseYaml(text) ?? {}) as T;
    } catch (err) {
        process.stderr.write(`[config] Warning: Failed to parse ${path}: ${err}\n`);
        return {} as T;
    }
}

export function loadModels(): LoadedModel[] {
    const raw: RawModelsYaml = safeLoad(MODELS_YML);
    const providers = raw.providers ?? {};
    const result: LoadedModel[] = [];
    for (const [providerName, cfg] of Object.entries(providers)) {
        if (!cfg.models || !Array.isArray(cfg.models)) continue;
        for (const model of cfg.models) {
            result.push({ provider: providerName, providerConfig: cfg, modelSpec: model });
        }
    }
    return result;
}

export function loadConfig(): Partial<RawConfigYaml> {
    return safeLoad(CONFIG_YML);
}

export function buildPiModel(loaded: LoadedModel): Model<Api> {
    const { provider, providerConfig, modelSpec } = loaded;
    const api = (providerConfig.api ?? "openai-completions") as Api;
    const inputTypes = (modelSpec.input ?? ["text"]) as ("text" | "image")[];
    return buildModel({
        provider,
        id: modelSpec.id,
        name: modelSpec.name ?? modelSpec.id,
        api,
        baseUrl: providerConfig.baseUrl ?? "https://api.openai.com/v1",
        reasoning: false,
        input: inputTypes,
        cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
        contextWindow: modelSpec.contextWindow ?? 32768,
        maxTokens: modelSpec.maxTokens ?? 16384,
    }) as Model<Api>;
}

export function resolveModelRole(role: string): LoadedModel | undefined {
    const parts = role.split("/");
    if (parts.length < 2) return undefined;
    const provider = parts[0]!;
    const modelId = parts.slice(1).join("/");
    const allModels = loadModels();
    return allModels.find(m => m.provider === provider && m.modelSpec.id === modelId);
}

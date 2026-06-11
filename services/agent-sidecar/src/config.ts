/**
 * Load omp configuration from ~/.omp/agent/models.yml
 * and build pi-catalog Model objects for custom providers.
 */
import { readFileSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { load as parseYaml } from "js-yaml";
import { buildModel } from "@oh-my-pi/pi-catalog/build";
import type { Api, Model } from "@oh-my-pi/pi-ai";

const OMP_CONFIG_DIR = join(homedir(), ".omp", "agent");
const MODELS_YML = join(OMP_CONFIG_DIR, "models.yml");
const CONFIG_YML = join(OMP_CONFIG_DIR, "config.yml");

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

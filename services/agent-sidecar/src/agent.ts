/**
 * Agent wrapper — manages the pi-agent-core Agent instance.
 */
import { Agent, type AgentEvent, type AgentTool } from "@oh-my-pi/pi-agent-core";
import type { Api, Model } from "@oh-my-pi/pi-ai";
import { buildPiModel, loadConfig, resolveModelRole, loadCompanionConfig, saveCompanionConfig, type CompanionConfig } from "./config";
import { sandboxResolve, hasDangerousChars, isHighRisk, logAudit } from "./sandbox";

// ── DuckDuckGo web search (zero-config, always available) ─────────────

async function webSearch(query: string) {
    // Try DuckDuckGo first (may be blocked in some regions)
    try {
        const q = encodeURIComponent(query);
        const resp = await fetch(`https://api.duckduckgo.com/?q=${q}&format=json`, { signal: AbortSignal.timeout(1500) });
        const data: any = await resp.json();
        let text = "";
        if (data.AbstractText) text += `Summary: ${data.AbstractText}\n`;
        if (data.AbstractURL) text += `Source: ${data.AbstractURL}\n`;
        if (data.RelatedTopics?.length) {
            text += "\nRelated:\n";
            for (const t of data.RelatedTopics.slice(0, 5)) {
                if (t.Text) text += `- ${t.Text}\n`;
            }
        }
        if (text) return { content: [{ type: "text" as const, text: text.trim() }] };
    } catch { /* fall through to Bing */ }

    // Fallback: Bing search (accessible from China)
    try {
        const q = encodeURIComponent(query);
        const resp = await fetch(`https://www.bing.com/search?q=${q}&setlang=en`, {
            signal: AbortSignal.timeout(2000),
            headers: { "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36" },
        });
        const html = await resp.text();
        // Extract snippets from Bing results
        const snippets: string[] = [];
        const re = /<li class="b_algo">[\s\S]*?<h2><a[^>]*>([\s\S]*?)<\/a>[\s\S]*?<p[^>]*>([\s\S]*?)<\/p>/gi;
        let match;
        while ((match = re.exec(html)) !== null && snippets.length < 5) {
            const title = match[1].replace(/<[^>]+>/g, "").trim();
            const desc = match[2].replace(/<[^>]+>/g, "").trim();
            snippets.push(`${title}: ${desc}`);
        }
        if (snippets.length > 0) {
            return { content: [{ type: "text" as const, text: `Bing results for "${query}":\n${snippets.join("\n")}` }] };
        }
    } catch { /* both failed */ }

    return { content: [{ type: "text" as const, text: `Search unavailable for "${query}". Try a different query or check internet.` }] };
}

// ── File & system tools (Bun native) ───────────────────────────────────

import { readFileSync, writeFileSync, existsSync, unlinkSync, rmdirSync, statSync, mkdirSync, readdirSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import { homedir } from "node:os";
import { Glob } from "bun";
import { MemoryManager } from "./memory-manager";

const TOOL_READ: AgentTool = {
    name: "read",
    label: "Read File",
    description: "Read the contents of a file. Use this to check file contents, read code, or inspect documents.",
    parameters: { type: "object", properties: { path: { type: "string", description: "Path to the file" } }, required: ["path"] },
    execute: async (_id, params: any) => {
        try {
            const text = readFileSync(params.path, "utf-8").slice(0, 20000);
            return { content: [{ type: "text" as const, text: text || "(empty file)" }] };
        } catch (err: any) {
            return { content: [{ type: "text" as const, text: `Read error: ${err.message}` }] };
        }
    },
};

const TOOL_WRITE: AgentTool = {
    name: "write",
    label: "Write File",
    description: "Write content to a file. Creates the file if it doesn't exist, overwrites if it does.",
    parameters: {
        type: "object",
        properties: { path: { type: "string" }, content: { type: "string" } },
        required: ["path", "content"],
    },
    execute: async (_id, params: any) => {
        try {
            writeFileSync(params.path, params.content, "utf-8");
            return { content: [{ type: "text" as const, text: `Wrote ${params.content.length} bytes to ${params.path}` }] };
        } catch (err: any) {
            return { content: [{ type: "text" as const, text: `Write error: ${err.message}` }] };
        }
    },
};

const TOOL_SEARCH: AgentTool = {
    name: "search",
    label: "Search Files",
    description: "Search for a text pattern in files under a directory. Returns matching files with line numbers.",
    parameters: {
        type: "object",
        properties: {
            pattern: { type: "string", description: "Text or regex to search for" },
            path: { type: "string", description: "Directory to search in (default: current directory)" },
        },
        required: ["pattern"],
    },
    execute: async (_id, params: any) => {
        try {
            const pattern = params.pattern;
            const dir = params.path || ".";
            const glob = new Glob("**/*");
            let output = "";
            for (const file of glob.scanSync({ cwd: dir, absolute: true })) {
                if (file.length > 500_000) continue; // skip large files
                try {
                    const content = readFileSync(file, "utf-8");
                    const lines = content.split("\n");
                    for (let i = 0; i < lines.length; i++) {
                        if (lines[i].includes(pattern)) {
                            output += `${file}:${i + 1}: ${lines[i].trim().slice(0, 200)}\n`;
                            if (output.length > 8000) { output += "...(truncated)\n"; break; }
                        }
                    }
                } catch {}
                if (output.length > 8000) break;
            }
            return { content: [{ type: "text" as const, text: output || `No matches for "${pattern}"` }] };
        } catch (err: any) {
            return { content: [{ type: "text" as const, text: `Search error: ${err.message}` }] };
        }
    },
};

const TOOL_FIND: AgentTool = {
    name: "find",
    label: "Find Files",
    description: "Find files matching a glob pattern. Use this to locate files by name.",
    parameters: {
        type: "object",
        properties: {
            pattern: { type: "string", description: "Glob pattern (e.g. **/*.ts, *.json)" },
            path: { type: "string", description: "Directory to search in (default: current directory)" },
        },
        required: ["pattern"],
    },
    execute: async (_id, params: any) => {
        try {
            const dir = params.path || ".";
            const glob = new Glob(params.pattern);
            const results: string[] = [];
            for (const file of glob.scanSync({ cwd: dir, absolute: true })) {
                results.push(file);
                if (results.length >= 100) break;
            }
            const text = results.length > 0 ? results.join("\n") : `No files matching "${params.pattern}"`;
            return { content: [{ type: "text" as const, text }] };
        } catch (err: any) {
            return { content: [{ type: "text" as const, text: `Find error: ${err.message}` }] };
        }
    },
};

const TOOL_BASH: AgentTool = {
    name: "bash",
    label: "Run Command",
    description: "Execute a shell command and return its output. Use for system operations. Avoid destructive commands.",
    parameters: {
        type: "object",
        properties: { command: { type: "string", description: "Shell command to execute" } },
        required: ["command"],
    },
    execute: async (_id, params: any) => {
        try {
            const output = execSync(params.command, { timeout: 30000, maxBuffer: 100 * 1024, encoding: "utf-8", shell: process.env.ComSpec || "cmd.exe" });
            return { content: [{ type: "text" as const, text: output || "(no output)" }] };
        } catch (err: any) {
            return { content: [{ type: "text" as const, text: `Command error: ${err.stderr || err.message}` }] };
        }
    },
};

export interface AgentCallbacks {
    onToken: (token: string) => void;
    onToolStart: (name: string) => void;
    onToolEnd: (name: string, result: string) => void;
    onDone: (text: string) => void;
    onError: (message: string) => void;
}

// ── Session / Conversation types ──────────────────────────────────────

export interface ConversationMeta {
    id: string;
    title: string;
    createdAt: string;
    updatedAt: string;
    messageCount: number;
}

function generateId(): string {
    // crypto.randomUUID available in Bun
    if (typeof crypto !== "undefined" && crypto.randomUUID) {
        return crypto.randomUUID();
    }
    // Fallback: timestamp + random
    return `${Date.now()}-${Math.random().toString(36).slice(2, 10)}`;
}

function nowISO(): string {
    return new Date().toISOString();
}

// ── Sandbox tools ──────────────────────────────────────────────────────

// ── Emotion & think-tag parsing ────────────────────────────────────────

/** Map emotion labels to Haru expression IDs (F01–F08). */
const EMOTION_MAP: Record<string, string> = {
    happy: "F01",
    sad: "F02",
    angry: "F03",
    surprised: "F04",
    shy: "F05",
    fear: "F02",
    joy: "F01",
    neutral: "",
};

const EMOTION_KEYS = Object.keys(EMOTION_MAP).join("|");
const EMOTION_REGEX = new RegExp(`\\[(${EMOTION_KEYS})\\]`, "gi");

/** Parse emotion tags from text, return { cleanText, emotions }. */
export function parseEmotions(text: string): { cleanText: string; emotions: string[] } {
    const t = String(text ?? "");
    const emotions: string[] = [];
    const cleanText = t.replace(EMOTION_REGEX, (_match, tag) => {
        const exprId = EMOTION_MAP[(tag as string).toLowerCase()];
        if (exprId) emotions.push(exprId);
        return "";
    });
    return { cleanText: cleanText.replace(/\s{2,}/g, " ").trim(), emotions };
}

/** Parse <think>...</think> tags: wrap inner content in markdown italic. */
export function parseThinkTags(text: string): { displayText: string; ttsText: string } {
    const thinkRegex = /<think>([\s\S]*?)<\/think>/gi;
    let ttsText: string = String(text ?? "");
    let displayText: string = String(text ?? "");
    // For TTS: remove think content entirely
    ttsText = ttsText.replace(thinkRegex, "");
    // For display: replace <think>...</think> with *...* (italic markdown)
    displayText = displayText.replace(thinkRegex, (_match, inner) => `*${inner.trim()}*`);
    return { displayText: displayText.trim(), ttsText: ttsText.replace(/\s{2,}/g, " ").trim() };
}

/** Build the emotion tag instruction string for the system prompt. */
export function emotionPromptFragment(): string {
    const tags = Object.keys(EMOTION_MAP).filter(k => k !== "neutral" && EMOTION_MAP[k] !== EMOTION_MAP.neutral).join(", ");
    return `You can add emotion tags to your responses to control your facial expression. Available tags: [${tags}]. Use them naturally — like "[happy] Hello!" or "[surprised] That's interesting! [smirk] But I have a secret.".`;
}

/** Prompt to convert math/symbols to spoken form for natural TTS output. */
export const SPEAKABLE_PROMPT = `Make your responses speakable by TTS. Convert math formulas, numbers, and symbols to their spoken form (e.g., "5x^2 + 3x - 2" → "five X squared plus three X minus two", "$50.25" → "fifty dollars and twenty-five cents", "H₂O" → "H two O"). Avoid markdown formatting symbols in spoken content.`;

/** Prompt to make tool calling decisive — use tools without asking. */
export const TOOL_GUIDANCE_PROMPT = `If a tool is needed, proactively use it without asking the user directly. You can use at most one sentence to explain before using a tool. Do NOT use bash or ping to test network connectivity — web_search handles this internally.`;

function makeSandboxTools(sandboxRoot: string): AgentTool[] {
    const safe = (rel: string) => sandboxResolve(rel || ".", sandboxRoot);

    return [
        {
            name: "sandbox_list",
            label: "List Sandbox",
            description: "List files and directories in the sandbox.",
            parameters: { type: "object", properties: { path: { type: "string", description: "Relative path (default: root)" } } },
            execute: async (_id: string, params: any) => {
                try {
                    const dir = safe(params.path || ".");
                    const glob = new Glob("*");
                    const entries = Array.from(glob.scanSync({ cwd: dir, absolute: false }));
                    const items: Array<{ name: string; type: string; size: number }> = [];
                    for (const e of entries) {
                        const full = `${dir}/${e}`.replace(/\/+/g, "/");
                        let type = "file", size = 0;
                        try { size = readFileSync(full, "utf-8").length; } catch { type = "directory"; }
                        items.push({ name: e, type, size });
                    }
                    logAudit("sandbox_list", `path=${params.path || "."}, entries=${items.length}`);
                    return { content: [{ type: "text" as const, text: JSON.stringify({ entries: items }, null, 2) }] };
                } catch (err: any) {
                    return { content: [{ type: "text" as const, text: `sandbox_list error: ${err.message}` }] };
                }
            },
        },
        {
            name: "sandbox_read",
            label: "Read Sandbox File",
            description: "Read a file inside the sandbox.",
            parameters: { type: "object", properties: { path: { type: "string" } }, required: ["path"] },
            execute: async (_id: string, params: any) => {
                try {
                    const path = safe(params.path);
                    const text = readFileSync(path, "utf-8").slice(0, 20000);
                    logAudit("sandbox_read", `path=${params.path}`);
                    return { content: [{ type: "text" as const, text: JSON.stringify({ content: text }) }] };
                } catch (err: any) {
                    return { content: [{ type: "text" as const, text: `sandbox_read error: ${err.message}` }] };
                }
            },
        },
        {
            name: "sandbox_write",
            label: "Write Sandbox File",
            description: "Write content to a file inside the sandbox.",
            parameters: { type: "object", properties: { path: { type: "string" }, content: { type: "string" } }, required: ["path", "content"] },
            execute: async (_id: string, params: any) => {
                try {
                    const path = safe(params.path);
                    writeFileSync(path, params.content, "utf-8");
                    logAudit("sandbox_write", `path=${params.path}, bytes=${params.content.length}`);
                    return { content: [{ type: "text" as const, text: JSON.stringify({ path, size: params.content.length }) }] };
                } catch (err: any) {
                    return { content: [{ type: "text" as const, text: `sandbox_write error: ${err.message}` }] };
                }
            },
        },
        {
            name: "sandbox_delete",
            label: "Delete Sandbox File",
            description: "Delete a file or empty directory inside the sandbox.",
            parameters: { type: "object", properties: { path: { type: "string" } }, required: ["path"] },
            execute: async (_id: string, params: any) => {
                try {
                    const path = safe(params.path);
                    if (existsSync(path)) {
                        const s = statSync(path);
                        if (s.isDirectory()) rmdirSync(path);
                        else unlinkSync(path);
                    }
                    logAudit("sandbox_delete", `path=${params.path}`);
                    return { content: [{ type: "text" as const, text: JSON.stringify({ deleted: params.path }) }] };
                } catch (err: any) {
                    return { content: [{ type: "text" as const, text: `sandbox_delete error: ${err.message}` }] };
                }
            },
        },
        {
            name: "sandbox_execute",
            label: "Execute in Sandbox",
            description: "Run a command inside the sandbox directory. High-risk commands are blocked.",
            parameters: { type: "object", properties: { command: { type: "string" } }, required: ["command"] },
            execute: async (_id: string, params: any) => {
                try {
                    const cmd = params.command as string;
                    if (hasDangerousChars(cmd)) {
                        throw new Error("command contains dangerous characters");
                    }
                    if (isHighRisk(cmd)) {
                        throw new Error(`'${cmd.split(/\s+/)[0]}' is a high-risk command.`);
                    }
                    logAudit("sandbox_execute", `command=${cmd}`);
                    const output = execSync(cmd, { encoding: "utf-8", timeout: 30_000, cwd: sandboxRoot }).slice(0, 4000);
                    return { content: [{ type: "text" as const, text: output || "(no output)" }] };
                } catch (err: any) {
                    return { content: [{ type: "text" as const, text: `sandbox_execute error: ${err.message}` }] };
                }
            },
        },
    ];
}

// ── Web tools (DuckDuckGo — free, zero config) ─────────────────────

/** Search the web via DuckDuckGo (free, zero config). Always works. */
const WEB_SEARCH_TOOL: AgentTool = {
    name: "web_search",
    label: "Web Search",
    description: "Search the internet using DuckDuckGo. Call this whenever you need current or factual information. Works without any API keys — just call it.",
    parameters: {
        type: "object",
        properties: {
            query: { type: "string", description: "Search query" },
            limit: { type: "number", description: "Max results (optional)" },
        },
        required: ["query"],
    },
    execute: async (_toolCallId: string, params: any) => {
        return await webSearch(params.query);
    },
};

/** Fetch and read the text content of a URL. */
const WEB_FETCH_TOOL: AgentTool = {
    name: "web_fetch",
    label: "Web Fetch",
    description: "Fetch and read the text content of a specific URL. Use this when you need detailed info from a known page.",
    parameters: {
        type: "object",
        properties: {
            url: { type: "string", description: "Full URL to fetch (e.g. https://example.com/page)" },
        },
        required: ["url"],
    },
    execute: async (_toolCallId: string, params: any) => {
        const url = String(params.url ?? "");
        if (!url.startsWith("http://") && !url.startsWith("https://")) {
            return { content: [{ type: "text" as const, text: "Only http:// and https:// URLs are allowed." }] };
        }
        try {
            const resp = await fetch(url, { signal: AbortSignal.timeout(15000) });
            const html = await resp.text();
            // Basic HTML-to-text: strip tags, condense whitespace
            const text = html
                .replace(/<script[^>]*>[\s\S]*?<\/script>/gi, "")
                .replace(/<style[^>]*>[\s\S]*?<\/style>/gi, "")
                .replace(/<[^>]+>/g, " ")
                .replace(/\s+/g, " ")
                .trim()
                .slice(0, 8000);
            return { content: [{ type: "text" as const, text: text || "(empty page)" }] };
        } catch (err: any) {
            return { content: [{ type: "text" as const, text: `Fetch failed: ${err.message}` }] };
        }
    },
};

export class AgentManager {
    private agent: Agent;
    private model: Model<Api>;
    private apiKey: string;
    private companionConfig: CompanionConfig;
    private messageHistory: Array<{ role: string; content: string }> = [];
    private convDir: string;
    private currentConversationId: string;
    private autoTitled = false;
    private memory: MemoryManager;

    constructor() {
        this.companionConfig = loadCompanionConfig();
        // Build model: CompanionConfig selects provider/model, omp models.yml provides API config
        const llmCfg = this.companionConfig.llm;
        const role = llmCfg.provider && llmCfg.model
            ? `${llmCfg.provider}/${llmCfg.model}`
            : (loadConfig().modelRoles?.default ?? "sensenova/mimo-v2.5");
        const resolved = resolveModelRole(role);
        if (resolved) {
            const key = llmCfg.key || this.companionConfig.default_api_key;
            if (key) resolved.providerConfig.apiKey = key;
            if (llmCfg.url) resolved.providerConfig.baseUrl = llmCfg.url;
            this.model = buildPiModel(resolved);
            this.apiKey = resolved.providerConfig.apiKey ?? "";
        } else {
            // Fallback: omp models.yml missing — build from CompanionConfig
            const provider = llmCfg.provider || "sensenova";
            const modelId = llmCfg.model || "mimo-v2.5";
            const key = llmCfg.key || this.companionConfig.default_api_key || "";
            this.apiKey = key;
            this.model = buildPiModel({
                provider,
                providerConfig: { baseUrl: llmCfg.url || "https://api.siliconflow.cn/v1", apiKey: key, api: "openai-completions", models: [{ id: modelId, name: modelId, input: ["text"], contextWindow: 32768, maxTokens: 16384 }] },
                modelSpec: { id: modelId, name: modelId, input: ["text"], contextWindow: 32768, maxTokens: 16384 },
            });
        }
        this.convDir = join(homedir(), ".companion", "conversations");
        // Init Mnemopi memory (FTS-only, no embeddings needed)
        this.memory = new MemoryManager();
        // Resume last conversation or create default
        const lastId = this.loadIndexCurrent();
        if (lastId && existsSync(join(this.convDir, `${lastId}.json`))) {
            this.currentConversationId = lastId;
            this.loadConversation();
            // Determine if already auto-titled
            this.autoTitled = this.conversationHasMessages();
        } else {
            this.currentConversationId = this.createConversationInternal("New Chat");
            this.messageHistory = [];
            this.autoTitled = false;
        }
        this.agent = this.createAgent();
    }

    private createAgent(): Agent {
        const sp = this.companionConfig.custom_system_prompt
            || "Companion — a helpful desktop AI assistant.";
        const agent = new Agent({
            initialState: {
                systemPrompt: [sp],
                model: this.model as any,
            },
            getApiKey: () => this.apiKey,
        });
        // All tools registered at startup — no runtime registration needed
        const sandboxTools = makeSandboxTools(this.companionConfig.sandbox_path);
        agent.setTools([
            ...sandboxTools,
            WEB_SEARCH_TOOL, WEB_FETCH_TOOL,
            TOOL_READ, TOOL_WRITE, TOOL_SEARCH, TOOL_FIND, TOOL_BASH,
            {
                name: "memory_retain",
                label: "Remember Fact",
                description: "Store an important fact about the user or context into long-term memory. Use this when the user shares preferences, personal info, or important decisions. The fact should be a concise summary (one sentence).",
                parameters: {
                    type: "object",
                    properties: {
                        fact: { type: "string", description: "A concise fact to remember (e.g. 'User prefers dark mode', 'Project uses Rust+Tauri')" },
                    },
                    required: ["fact"],
                },
                execute: async (_id: string, params: any) => {
                    this.memory.retainMemory(params.fact);
                    return { content: [{ type: "text" as const, text: `✓ Remembered: ${params.fact}` }] };
                },
            },
        ]);
        return agent;
    }

    getCompanionConfig(): CompanionConfig {
        return this.companionConfig;
    }

    updateCompanionConfig(partial: Partial<CompanionConfig>): CompanionConfig {
        this.companionConfig = { ...this.companionConfig, ...partial };
        saveCompanionConfig(this.companionConfig);
        // Rebuild model if LLM config changed
        const llmCfg = this.companionConfig.llm;
        const needRebuild = partial.llm || partial.custom_system_prompt !== undefined || partial.sandbox_path !== undefined || partial.default_api_key !== undefined;
        if (needRebuild) {
            if (partial.llm || partial.default_api_key !== undefined) {
                const cfg = this.companionConfig;
                const role = cfg.llm.provider && cfg.llm.model
                    ? `${cfg.llm.provider}/${cfg.llm.model}`
                    : (loadConfig().modelRoles?.default ?? "sensenova/mimo-v2.5");
                const resolved = resolveModelRole(role);
                if (resolved) {
                    const key = cfg.llm.key || cfg.default_api_key;
                    if (key) resolved.providerConfig.apiKey = key;
                    if (cfg.llm.url) resolved.providerConfig.baseUrl = cfg.llm.url;
                    this.model = buildPiModel(resolved);
                    this.apiKey = resolved.providerConfig.apiKey ?? "";
                }
            }
            this.agent = this.createAgent();
        }
        return this.companionConfig;
    }

    getModelInfo(): { provider: string; model: string } {
        return { provider: this.model.provider, model: this.model.id };
    }

    getApiKeyValue(): string {
        return this.apiKey;
    }

    clearHistory(): void {
        this.messageHistory = [];
        this.agent.clearMessages();
        try { unlinkSync(join(this.convDir, `${this.currentConversationId}.json`)); } catch {}
        this.updateIndexMeta(this.currentConversationId, { messageCount: 0 });
    }

    getHistory(): Array<{ role: string; content: string }> {
        return this.messageHistory;
    }

    // ── Index file helpers ────────────────────────────────────────────

    private indexPath(): string {
        return join(this.convDir, "index.json");
    }

    private loadIndex(): Array<ConversationMeta> {
        try {
            const path = this.indexPath();
            if (existsSync(path)) {
                return JSON.parse(readFileSync(path, "utf-8"));
            }
        } catch {}
        return [];
    }

    private saveIndex(meta: Array<ConversationMeta>): void {
        try {
            if (!existsSync(this.convDir)) mkdirSync(this.convDir, { recursive: true });
            writeFileSync(this.indexPath(), JSON.stringify(meta, null, 2), "utf-8");
        } catch {}
    }

    private loadIndexCurrent(): string | null {
        const meta = this.loadIndex();
        // Return the most recently updated conversation
        if (meta.length === 0) return null;
        meta.sort((a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime());
        return meta[0].id;
    }

    private updateIndexMeta(id: string, partial: Partial<ConversationMeta>): void {
        const meta = this.loadIndex();
        const idx = meta.findIndex(m => m.id === id);
        if (idx >= 0) {
            meta[idx] = { ...meta[idx], ...partial, updatedAt: nowISO() };
        }
        this.saveIndex(meta);
    }

    private conversationHasMessages(): boolean {
        return this.messageHistory.length > 0;
    }

    private saveConversation(): void {
        try {
            if (!existsSync(this.convDir)) mkdirSync(this.convDir, { recursive: true });
            const path = join(this.convDir, `${this.currentConversationId}.json`);
            writeFileSync(path, JSON.stringify(this.messageHistory, null, 2), "utf-8");
            // Update index
            this.updateIndexMeta(this.currentConversationId, {
                messageCount: this.messageHistory.length,
            });
        } catch {}
    }

    private loadConversation(): void {
        try {
            const path = join(this.convDir, `${this.currentConversationId}.json`);
            if (existsSync(path)) {
                this.messageHistory = JSON.parse(readFileSync(path, "utf-8"));
            } else {
                this.messageHistory = [];
            }
        } catch {
            this.messageHistory = [];
        }
    }

    // ── Auto-title from first user message ────────────────────────────

    private maybeAutoTitle(userMessage: string): void {
        if (this.autoTitled) return;
        const title = userMessage.slice(0, 40).replace(/\n/g, " ").trim();
        if (!title) return;
        this.renameConversation(this.currentConversationId, title);
        this.autoTitled = true;
    }

    // ── Session CRUD ──────────────────────────────────────────────────

    private createConversationInternal(title: string): string {
        const id = generateId();
        const meta = this.loadIndex();
        meta.push({
            id,
            title,
            createdAt: nowISO(),
            updatedAt: nowISO(),
            messageCount: 0,
        });
        this.saveIndex(meta);
        // Create empty messages file
        writeFileSync(join(this.convDir, `${id}.json`), "[]", "utf-8");
        return id;
    }

    listConversations(): Array<ConversationMeta> {
        return this.loadIndex().sort(
            (a, b) => new Date(b.updatedAt).getTime() - new Date(a.updatedAt).getTime()
        );
    }

    getCurrentConversationId(): string {
        return this.currentConversationId;
    }

    /** Create a new empty conversation and switch to it. */
    createConversation(title?: string): ConversationMeta {
        // Save current first
        this.saveConversation();
        // Create new
        const id = this.createConversationInternal(title || "New Chat");
        this.currentConversationId = id;
        this.messageHistory = [];
        this.autoTitled = false;
        this.agent.clearMessages();
        // Return the created meta
        const meta = this.loadIndex().find(m => m.id === id);
        return meta!;
    }

    /** Switch to an existing conversation, loading its messages. */
    switchConversation(id: string): { meta: ConversationMeta; messages: Array<{ role: string; content: string }> } | null {
        const meta = this.loadIndex().find(m => m.id === id);
        if (!meta) return null;
        // Save current
        this.saveConversation();
        // Switch
        this.currentConversationId = id;
        this.loadConversation();
        this.autoTitled = this.conversationHasMessages();
        // Reload agent context
        this.agent.clearMessages();
        // Replay history into agent (pi-agent-core doesn't have loadMessages, so we replay via prompt)
        // Actually: pi-agent-core's Agent doesn't have a way to bulk-load messages.
        // The history will be sent via the chat/chatStream call's history parameter.
        return { meta, messages: [...this.messageHistory] };
    }

    /** Delete a conversation and its message file. */
    deleteConversation(id: string): boolean {
        const meta = this.loadIndex();
        const idx = meta.findIndex(m => m.id === id);
        if (idx < 0) return false;
        // Remove from index
        meta.splice(idx, 1);
        this.saveIndex(meta);
        // Delete message file
        try { unlinkSync(join(this.convDir, `${id}.json`)); } catch {}
        // If this was the current conversation, switch to the most recent remaining
        if (this.currentConversationId === id) {
            if (meta.length > 0) {
                this.currentConversationId = meta[meta.length - 1].id;
                this.loadConversation();
                this.autoTitled = this.conversationHasMessages();
            } else {
                // No conversations left — create a new default
                this.currentConversationId = this.createConversationInternal("New Chat");
                this.messageHistory = [];
                this.autoTitled = false;
            }
            this.agent.clearMessages();
        }
        return true;
    }

    /** Rename a conversation. */
    renameConversation(id: string, title: string): boolean {
        const meta = this.loadIndex();
        const idx = meta.findIndex(m => m.id === id);
        if (idx < 0) return false;
        meta[idx].title = title;
        meta[idx].updatedAt = nowISO();
        this.saveIndex(meta);
        return true;
    }

    // ── Memory (Mnemopi) — delegates to MemoryManager ─────────────────

    private async recallMemories(query: string): Promise<string> {
        return this.memory.recallMemories(query);
    }

    private retainMemory(content: string): void {
        this.memory.retainMemory(content);
    }

    listMemories(): Array<{ id: string; content: string; timestamp: string }> {
        return this.memory.listMemories();
    }

    forgetMemory(id: string): boolean {
        return this.memory.forgetMemory(id);
    }

    async chat(message: string, _history?: Array<{ role: string; content: string }>, _systemPrompt?: string): Promise<{ text: string; history: Array<{ role: string; content: string }>; emotions?: string[] }> {
        // Recall relevant memories (2s timeout — don't block first token)
        const memoryCtx = await Promise.race([
            this.recallMemories(message),
            new Promise<string>(r => setTimeout(() => r(""), 2000)),
        ]);
        const augmentedMessage = memoryCtx ? `${memoryCtx}\n${message}` : message;

        return new Promise<{ text: string; history: Array<{ role: string; content: string }>; emotions?: string[] }>((resolve, reject) => {
            let fullText = "";
            const unsubscribe = this.agent.subscribe((event: AgentEvent) => {
                if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
                    fullText += String(event.assistantMessageEvent.delta ?? "");
                } else if (event.type === "agent_end") {
                    unsubscribe();
                    const rawText = fullText || "⚠️ The model returned an empty response. This may indicate an API error or a tool call with no follow-up text. Try asking again.";
                    // Parse emotion + think tags
                    const { displayText, ttsText } = parseThinkTags(rawText);
                    const { cleanText, emotions } = parseEmotions(displayText);
                    const text = cleanText || displayText;
                    this.maybeAutoTitle(message);
                    this.messageHistory.push({ role: "user", content: message });
                    this.messageHistory.push({ role: "assistant", content: text });
                    if (this.messageHistory.length > 50) {
                        this.messageHistory = this.messageHistory.slice(-50);
                    }
                    this.saveConversation();
                    resolve({
                        text,
                        history: this.messageHistory,
                        ...(emotions.length > 0 ? { emotions } : {}),
                    });
                }
            });
            this.agent.prompt(augmentedMessage, { toolChoice: undefined }).catch((err: Error) => {
                process.stderr.write(`[agent] chat prompt failed: ${err.message}\n`);
                unsubscribe();
                reject(err.message);
            });
        });
    }

    async chatStream(message: string, history?: Array<{ role: string; content: string }>, _systemPrompt?: string, callbacks?: AgentCallbacks): Promise<void> {
        // Recall relevant memories (1s timeout)
        const memoryCtx = await Promise.race([
            this.recallMemories(message),
            new Promise<string>(r => setTimeout(() => r(""), 1000)),
        ]);
        const augmentedMessage = memoryCtx ? `${memoryCtx}\n${message}` : message;

        // Replay history only when switching conversations (Agent keeps context between turns)
        if (history && history.length > 0 && this.messageHistory.length === 0) {
            this.agent.replaceMessages(history as any);
        }

        if (!callbacks) {
            try {
                await this.agent.prompt(augmentedMessage, { toolChoice: undefined });
            } catch (err: any) {
                process.stderr.write(`[agent] chatStream prompt failed: ${err.message}\n`);
            }
            return;
        }

        return new Promise<void>((resolve, reject) => {
            let fullText = "";
            let settled = false;

            const unsubscribe = this.agent.subscribe((event: AgentEvent) => {
                try {
                    switch (event.type) {
                        case "message_update":
                            if (event.assistantMessageEvent.type === "text_delta") {
                                const delta = String(event.assistantMessageEvent.delta ?? "");
                                fullText += delta;
                                callbacks!.onToken(delta);
                            } else if ((event.assistantMessageEvent as any).type === "reasoning_delta") {
                                // Reasoning models emit thinking tokens — capture as regular tokens
                                const delta = String((event.assistantMessageEvent as any).delta ?? "");
                                callbacks!.onToken(delta);
                            }
                            break;
                        case "tool_execution_start":
                            callbacks!.onToolStart(event.toolName);
                            break;
                        case "tool_execution_end":
                            callbacks!.onToolEnd(event.toolName, JSON.stringify(event.result));
                            break;
                        case "agent_end":
                            if (settled) break; settled = true;
                            clearTimeout(timer);
                            this.maybeAutoTitle(message);
                            if (!fullText && (event as any).messages?.length > 0) {
                                const lastMsg = (event as any).messages[(event as any).messages.length - 1];
                                const mc = lastMsg?.content;
                                fullText = typeof mc === "string" ? mc : (mc?.text || mc?.message || "");
                                // For reasoning models: use reasoning_content if content is empty
                                if (!fullText && lastMsg?.reasoning_content) {
                                    fullText = String(lastMsg.reasoning_content);
                                }
                            }
                            const text = fullText || "⚠️ Empty response";
                            this.messageHistory.push({ role: "user", content: message });
                            this.messageHistory.push({ role: "assistant", content: text });
                            if (this.messageHistory.length > 50) this.messageHistory = this.messageHistory.slice(-50);
                            this.saveConversation();
                            callbacks!.onDone(text);
                            unsubscribe();
                            resolve();
                            break;
                    }
                } catch (err) {
                    callbacks!.onError(String(err));
                    if (!settled) { settled = true; clearTimeout(timer); unsubscribe(); reject(err); }
                }
            });

            // 45s global timeout
            const timer = setTimeout(() => {
                if (settled) return; settled = true;
                const fallback = fullText || "⚠️ Request timed out after 45s.";
                this.messageHistory.push({ role: "user", content: message });
                this.messageHistory.push({ role: "assistant", content: fallback });
                this.saveConversation();
                callbacks!.onDone(fallback);
                unsubscribe();
                resolve();
            }, 45000);

            this.agent.prompt(augmentedMessage, { toolChoice: undefined }).catch((err: any) => {
                const msg = typeof err === "string" ? err : (err?.message || String(err));
                if (settled) return; settled = true;
                clearTimeout(timer);
                const errorText = `⚠️ API Error: ${msg}`;
                this.messageHistory.push({ role: "user", content: message });
                this.messageHistory.push({ role: "assistant", content: errorText });
                this.saveConversation();
                callbacks!.onDone(errorText);
                unsubscribe();
                reject(err);
            });
        });
    }

    setTools(tools: AgentTool[]): void {
        this.agent.setTools(tools);
    }
}

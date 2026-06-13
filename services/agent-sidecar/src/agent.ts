/**
 * Agent wrapper — manages the pi-agent-core Agent instance.
 */
import { Agent, type AgentEvent, type AgentTool } from "@oh-my-pi/pi-agent-core";
import type { Api, Model } from "@oh-my-pi/pi-ai";
import { buildPiModel, loadConfig, resolveModelRole, loadCompanionConfig, saveCompanionConfig, type CompanionConfig } from "./config";
import { sandboxResolve, hasDangerousChars, isHighRisk, logAudit } from "./sandbox";

// ── DuckDuckGo web search (zero-config, always available) ─────────────

async function duckDuckGoSearch(query: string) {
    try {
        const q = encodeURIComponent(query);
        const resp = await fetch(`https://api.duckduckgo.com/?q=${q}&format=json`);
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
        if (!text) text = `No results found for "${query}".`;
        return { content: [{ type: "text" as const, text: text.trim() }] };
    } catch (err: any) {
        return { content: [{ type: "text" as const, text: `Search failed: ${err.message}` }] };
    }
}

// ── File & system tools (Bun native) ───────────────────────────────────

import { readFileSync, writeFileSync, existsSync, unlinkSync, rmdirSync, statSync, mkdirSync } from "node:fs";
import { execSync } from "node:child_process";
import { join } from "node:path";
import { homedir } from "node:os";
import { Glob } from "bun";

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
    const emotions: string[] = [];
    const cleanText = text.replace(EMOTION_REGEX, (_match, tag) => {
        const exprId = EMOTION_MAP[(tag as string).toLowerCase()];
        if (exprId) emotions.push(exprId);
        return "";
    });
    return { cleanText: cleanText.replace(/\s{2,}/g, " ").trim(), emotions };
}

/** Parse <think>...</think> tags: wrap inner content in markdown italic. */
export function parseThinkTags(text: string): { displayText: string; ttsText: string } {
    const thinkRegex = /<think>([\s\S]*?)<\/think>/gi;
    let ttsText = text;
    let displayText = text;
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
export const TOOL_GUIDANCE_PROMPT = `If a tool is needed, proactively use it without asking the user directly. You can use at most one sentence to explain before using a tool.`;

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
        return await duckDuckGoSearch(params.query);
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
    private currentAbortController: AbortController | null = null;
    private pendingPrompt: Promise<void> | null = null;
    private resolvePending: (() => void) | null = null;

    constructor() {
        this.companionConfig = loadCompanionConfig();
        const config = loadConfig();
        const defaultRole = config.modelRoles?.default ?? "sensenova/mimo-v2.5";
        const resolved = resolveModelRole(defaultRole);
        if (!resolved) {
            throw new Error(`Could not resolve default model role: ${defaultRole}`);
        }
        this.model = buildPiModel(resolved);
        this.apiKey = resolved.providerConfig.apiKey ?? "";
        this.convDir = join(homedir(), ".companion", "conversations");
        this.loadConversation();
        this.agent = this.createAgent();
    }

    private createAgent(): Agent {
        // Build full system prompt once at creation — includes all guidance
        const sp = this.companionConfig.custom_system_prompt
            || "Companion — a helpful desktop AI assistant.";
        const fullPrompt = [
            sp,
            emotionPromptFragment(),
            `Wrap your inner thoughts and reasoning in <think>...</think> tags in EVERY response. These will be shown as italic text but NOT spoken. Example: "<think>Let me search for the weather data first...</think> Here's the weather:"`,
            SPEAKABLE_PROMPT,
            TOOL_GUIDANCE_PROMPT,
        ].join("\n\n");
        const agent = new Agent({
            initialState: {
                systemPrompt: [fullPrompt],
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
        ]);
        return agent;
    }

    getCompanionConfig(): CompanionConfig {
        return this.companionConfig;
    }

    updateCompanionConfig(partial: Partial<CompanionConfig>): CompanionConfig {
        this.companionConfig = { ...this.companionConfig, ...partial };
        saveCompanionConfig(this.companionConfig);
        return this.companionConfig;
    }

    getModelInfo(): { provider: string; model: string } {
        return { provider: this.model.provider, model: this.model.id };
    }

    getApiKeyValue(): string {
        return this.apiKey;
    }

    setModel(provider: string, modelId: string, baseUrl?: string, apiKey?: string): boolean {
        if (baseUrl || apiKey) {
            this.model = buildPiModel({
                provider,
                providerConfig: {
                    baseUrl: baseUrl ?? this.model.baseUrl,
                    apiKey: apiKey ?? this.apiKey,
                    api: "openai-completions",
                    models: [{
                        id: modelId,
                        name: modelId,
                        input: ["text"],
                        contextWindow: 32768,
                        maxTokens: 16384,
                    }],
                },
                modelSpec: {
                    id: modelId,
                    name: modelId,
                    input: ["text"],
                    contextWindow: 32768,
                    maxTokens: 16384,
                },
            });
            if (apiKey) this.apiKey = apiKey;
        } else {
            const resolved = resolveModelRole(`${provider}/${modelId}`);
            if (!resolved) return false;
            this.model = buildPiModel(resolved);
            this.apiKey = resolved.providerConfig.apiKey ?? "";
        }
        this.agent = this.createAgent();
        return true;
    }

    clearHistory(): void {
        this.messageHistory = [];
        this.agent.clearMessages();
        try { unlinkSync(join(this.convDir, "current.json")); } catch {}
    }

    getHistory(): Array<{ role: string; content: string }> {
        return this.messageHistory;
    }

    private saveConversation(): void {
        try {
            if (!existsSync(this.convDir)) mkdirSync(this.convDir, { recursive: true });
            writeFileSync(join(this.convDir, "current.json"), JSON.stringify(this.messageHistory, null, 2), "utf-8");
        } catch {}
    }

    private loadConversation(): void {
        try {
            const path = join(this.convDir, "current.json");
            if (existsSync(path)) {
                this.messageHistory = JSON.parse(readFileSync(path, "utf-8"));
            }
        } catch {}
    }

    async chat(message: string, _history?: Array<{ role: string; content: string }>, systemPrompt?: string): Promise<{ text: string; history: Array<{ role: string; content: string }>; emotions?: string[] }> {
        // Only update system prompt if caller explicitly passes a different one (rare — config changes)
        if (systemPrompt && systemPrompt.length > 0) {
            this.agent.setSystemPrompt([systemPrompt]);
        }

        return new Promise<{ text: string; history: Array<{ role: string; content: string }>; emotions?: string[] }>((resolve, reject) => {
            let fullText = "";
            const unsubscribe = this.agent.subscribe((event: AgentEvent) => {
                if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
                    fullText += event.assistantMessageEvent.delta;
                } else if (event.type === "agent_end") {
                    unsubscribe();
                    const rawText = fullText || "(no response)";
                    // Parse emotion + think tags
                    const { displayText, ttsText } = parseThinkTags(rawText);
                    const { cleanText, emotions } = parseEmotions(displayText);
                    const text = cleanText || displayText;
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
            this.agent.prompt(message, { toolChoice: undefined }).catch((err: Error) => {
                unsubscribe();
                reject(err.message);
            });
        });
    }

    async chatStream(message: string, history?: Array<{ role: string; content: string }>, systemPrompt?: string, callbacks?: AgentCallbacks): Promise<void> {
        if (systemPrompt) this.agent.setSystemPrompt([systemPrompt]);
        if (!callbacks) {
            await this.agent.prompt(message, { toolChoice: undefined });
            return;
        }

        return new Promise<void>((resolve, reject) => {
            let fullText = "";
            const unsubscribe = this.agent.subscribe((event: AgentEvent) => {
                try {
                    switch (event.type) {
                        case "message_update":
                            if (event.assistantMessageEvent.type === "text_delta") {
                                const delta = event.assistantMessageEvent.delta;
                                fullText += delta;
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
                            callbacks!.onDone(fullText);
                            unsubscribe();
                            resolve();
                            break;
                    }
                } catch (err) {
                    callbacks!.onError(String(err));
                    unsubscribe();
                    reject(err);
                }
            });
            this.agent.prompt(message, { toolChoice: undefined }).catch((err: Error) => {
                unsubscribe();
                callbacks?.onError(err.message);
                reject(err.message);
            });
        });
    }

    setTools(tools: AgentTool[]): void {
        this.agent.setTools(tools);
    }

    abort(): void {
        this.currentAbortController?.abort();
        this.currentAbortController = null;
    }
}

/**
 * Agent wrapper — manages the pi-agent-core Agent instance.
 */
import { Agent, type AgentEvent, type AgentTool } from "@oh-my-pi/pi-agent-core";
import type { Api, Model } from "@oh-my-pi/pi-ai";
import { buildPiModel, loadConfig, resolveModelRole, loadCompanionConfig, saveCompanionConfig, type CompanionConfig } from "./config";

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

import { readFileSync, writeFileSync, existsSync, unlinkSync, rmdirSync, statSync } from "node:fs";
import { execSync } from "node:child_process";
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
        agent.setTools([WEB_SEARCH_TOOL, WEB_FETCH_TOOL, TOOL_READ, TOOL_WRITE, TOOL_SEARCH, TOOL_FIND, TOOL_BASH]);
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
    }

    getHistory(): Array<{ role: string; content: string }> {
        return this.messageHistory;
    }

    async chat(message: string, _history?: Array<{ role: string; content: string }>, systemPrompt?: string): Promise<{ text: string; history: Array<{ role: string; content: string }> }> {
        if (systemPrompt && systemPrompt.length > 0) this.agent.setSystemPrompt([systemPrompt]);

        return new Promise<{ text: string; history: Array<{ role: string; content: string }> }>((resolve, reject) => {
            let fullText = "";
            const unsubscribe = this.agent.subscribe((event: AgentEvent) => {
                if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
                    fullText += event.assistantMessageEvent.delta;
                } else if (event.type === "agent_end") {
                    unsubscribe();
                    const text = fullText || "(no response)";
                    this.messageHistory.push({ role: "user", content: message });
                    this.messageHistory.push({ role: "assistant", content: text });
                    if (this.messageHistory.length > 50) {
                        this.messageHistory = this.messageHistory.slice(-50);
                    }
                    resolve({ text, history: this.messageHistory });
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

    /**
     * Register tools from JSON definitions (from Rust).
     * Each tool def has name, description, parameters (JSON Schema).
     */
    registerToolsFromDefs(toolDefs: Array<{
        name: string;
        description: string;
        parameters: Record<string, unknown>;
    }>, sandboxRoot: string): void {
        // Resolve relative paths against the sandbox root.
        const resolve = (rel: string): string => {
            return `${sandboxRoot}/${rel}`.replace(/\/+/g, "/");
        };
        const tools: AgentTool[] = toolDefs.map((def) => ({
            name: def.name,
            label: def.name,
            description: def.description,
            parameters: def.parameters as any,
            execute: async (_toolCallId: string, params: any) => {
                try {
                    switch (def.name) {
                        case "sandbox_list": {
                            const dir = resolve(params.path || ".");
                            const glob = new Glob("*");
                            const entries = Array.from(glob.scanSync({ cwd: dir, absolute: false }));
                            const items: Array<{ name: string; type: string; size: number }> = [];
                            for (const e of entries) {
                                const full = `${dir}/${e}`.replace(/\/+/g, "/");
                                let type: string = "file";
                                let size = 0;
                                try {
                                    const stat = readFileSync(full, "utf-8");
                                    size = stat.length;
                                } catch {
                                    type = "directory";
                                }
                                items.push({ name: e, type, size });
                            }
                            return { content: [{ type: "text" as const, text: JSON.stringify({ entries: items }, null, 2) }] };
                        }
                        case "sandbox_read": {
                            const path = resolve(params.path);
                            const text = readFileSync(path, "utf-8").slice(0, 20000);
                            return { content: [{ type: "text" as const, text: JSON.stringify({ content: text }) }] };
                        }
                        case "sandbox_write": {
                            const path = resolve(params.path);
                            writeFileSync(path, params.content, "utf-8");
                            return { content: [{ type: "text" as const, text: JSON.stringify({ path, size: params.content.length }) }] };
                        }
                        case "sandbox_delete": {
                            const path = resolve(params.path);
                            if (existsSync(path)) {
                                const s = statSync(path);
                                if (s.isDirectory()) rmdirSync(path);
                                else unlinkSync(path);
                            }
                            return { content: [{ type: "text" as const, text: JSON.stringify({ deleted: params.path }) }] };
                        }
                        case "sandbox_execute": {
                            const cmd = params.command as string;
                            if (/[;&|$`\n\r]/.test(cmd)) {
                                return { content: [{ type: "text" as const, text: "sandbox_execute error: command contains dangerous characters" }] };
                            }
                            const output = execSync(cmd, { encoding: "utf-8", timeout: 30_000, cwd: sandboxRoot }).slice(0, 4000);
                            return { content: [{ type: "text" as const, text: output || "(no output)" }] };
                        }
                        default:
                            return { content: [{ type: "text" as const, text: `Tool ${def.name} is not implemented.` }] };
                    }
                } catch (err: any) {
                    return { content: [{ type: "text" as const, text: `${def.name} error: ${err.message}` }] };
                }
            },
        })) as unknown as AgentTool[];

        this.agent.setTools(tools);
    }

    /**
     * Load omp's built-in read/write/search tools.
     * This registers tools that come from @oh-my-pi/pi-agent-core's default tool set.
     */
    loadOmnTools(): void {
        // Currently, pi-agent-core's Agent class doesn't auto-load omp tools.
        // Tools need to be explicitly set. For now, we use only explicitly registered tools.
        // This will be expanded in a future update.
    }

    abort(): void {
        this.currentAbortController?.abort();
        this.currentAbortController = null;
    }
}

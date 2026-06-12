/**
 * Agent wrapper — manages the pi-agent-core Agent instance.
 */
import { Agent, type AgentEvent, type AgentTool } from "@oh-my-pi/pi-agent-core";
import type { Api, Model } from "@oh-my-pi/pi-ai";
import { buildPiModel, loadConfig, resolveModelRole } from "./config";
import { runSearchQuery } from "@oh-my-pi/pi-coding-agent/web/search";

export interface AgentCallbacks {
    onToken: (token: string) => void;
    onToolStart: (name: string) => void;
    onToolEnd: (name: string, result: string) => void;
    onDone: (text: string) => void;
    onError: (message: string) => void;
}

// ── Web tools (powered by @oh-my-pi/pi-coding-agent) ────────────────────

/** Search the web using omp's multi-provider search (Brave, Perplexity, SearXNG, etc.). */
const WEB_SEARCH_TOOL: AgentTool = {
    name: "web_search",
    label: "Web Search",
    description: "Search the internet for current information. Supports multiple search providers configured in ~/.omp/agent/.",
    parameters: {
        type: "object",
        properties: {
            query: { type: "string", description: "Search query" },
            limit: { type: "number", description: "Max results (optional)" },
        },
        required: ["query"],
    },
    execute: async (_toolCallId: string, params: any) => {
        try {
            const result = await runSearchQuery({
                query: params.query,
                limit: params.limit ?? 5,
            });
            return { content: result.content };
        } catch (err: any) {
            return { content: [{ type: "text" as const, text: `Search failed: ${err.message}` }] };
        }
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
    private currentAbortController: AbortController | null = null;
    private pendingPrompt: Promise<void> | null = null;
    private resolvePending: (() => void) | null = null;

    constructor() {
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
        const agent = new Agent({
            initialState: {
                systemPrompt: ["You are Companion, a helpful cross-platform desktop AI assistant. You have TWO tools available: web_search and web_fetch. Use them for ANY question about current events, facts you're unsure about, news, or web content. NEVER say you cannot access the internet or search the web — you can, just call web_search. If the user asks something you don't know, search first."],
                model: this.model as any,
            },
            getApiKey: () => this.apiKey,
        });
        agent.setTools([WEB_SEARCH_TOOL, WEB_FETCH_TOOL]);
        return agent;
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
        this.agent.clearMessages();
    }

    async chat(message: string, history?: Array<{ role: string; content: string }>, systemPrompt?: string): Promise<string> {
        if (systemPrompt) this.agent.setSystemPrompt([systemPrompt]);
        // Only clear if explicit clear is requested (via clearHistory from Rust)
        // pi-agent-core's Agent internally manages conversation state across turns.

        return new Promise<string>((resolve, reject) => {
            let fullText = "";
            const unsubscribe = this.agent.subscribe((event: AgentEvent) => {
                if (event.type === "message_update" && event.assistantMessageEvent.type === "text_delta") {
                    fullText += event.assistantMessageEvent.delta;
                } else if (event.type === "agent_end") {
                    unsubscribe();
                    resolve(fullText || "(no response)");
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
    }>): void {
        const tools: AgentTool[] = toolDefs.map((def) => ({
            name: def.name,
            label: def.name,
            description: def.description,
            parameters: def.parameters as any,
            execute: async (toolCallId: string, params: any) => {
                // For now, sandbox tools are executed inside the sidecar process.
                // In production, we'd forward to the Rust side via a callback.
                return {
                    content: [{ type: "text" as const, text: `Tool ${def.name} executed with params: ${JSON.stringify(params)}` }],
                };
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

/**
 * Agent wrapper — manages the pi-agent-core Agent instance.
 */
import { Agent, type AgentEvent, type AgentTool } from "@oh-my-pi/pi-agent-core";
import type { Api, Model } from "@oh-my-pi/pi-ai";
import { buildPiModel, loadConfig, resolveModelRole } from "./config";

export interface AgentCallbacks {
    onToken: (token: string) => void;
    onToolStart: (name: string) => void;
    onToolEnd: (name: string, result: string) => void;
    onDone: (text: string) => void;
    onError: (message: string) => void;
}

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
        return new Agent({
            initialState: {
                systemPrompt: ["You are Companion, a helpful cross-platform desktop AI assistant."],
                model: this.model as any,
            },
            getApiKey: () => this.apiKey,
        });
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
        if (history && history.length > 0) this.agent.clearMessages();

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

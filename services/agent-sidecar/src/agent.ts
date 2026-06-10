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

/**
 * Manages an Agent instance that wraps pi-agent-core's Agent class.
 */
export class AgentManager {
    private agent: Agent;
    private model: Model<Api>;
    private apiKey: string;
    private currentAbortController: AbortController | null = null;

    constructor() {
        // Load default model from config
        const config = loadConfig();
        const defaultRole = config.modelRoles?.default ?? "sensenova/mimo-v2.5";
        const resolved = resolveModelRole(defaultRole);

        if (!resolved) {
            throw new Error(`Could not resolve default model role: ${defaultRole}`);
        }

        this.model = buildPiModel(resolved);
        this.apiKey = resolved.providerConfig.apiKey;

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

    /**
     * Get the model description for settings display.
     */
    getModelInfo(): { provider: string; model: string } {
        return {
            provider: this.model.provider,
            model: this.model.id,
        };
    }

    /**
     * Get current API key.
     */
    getApiKeyValue(): string {
        return this.apiKey;
    }

    /**
     * Switch to a different model.
     */
    setModel(provider: string, modelId: string, baseUrl?: string, apiKey?: string): boolean {
        if (baseUrl || apiKey) {
            // Custom model config
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
            // Resolve from models.yml
            const resolved = resolveModelRole(`${provider}/${modelId}`);
            if (!resolved) return false;
            this.model = buildPiModel(resolved);
            this.apiKey = resolved.providerConfig.apiKey;
        }

        this.agent = this.createAgent();
        return true;
    }

    /**
     * Clear conversation history.
     */
    clearHistory(): void {
        this.agent.clearMessages();
    }

    /**
     * Non-streaming chat — returns the full response text.
     */
    async chat(
        message: string,
        history?: Array<{ role: string; content: string }>,
        systemPrompt?: string,
    ): Promise<string> {
        this.currentAbortController = new AbortController();

        if (systemPrompt) {
            this.agent.setSystemPrompt([systemPrompt]);
        }

        // Restore history (optional — sidecar can be stateless)
        if (history && history.length > 0) {
            this.agent.clearMessages();
            // We don't replay history into Agent — only the most recent
            // message is used. The Rust side manages full history.
        }

        return new Promise<string>((resolve, reject) => {
            let fullText = "";

            this.agent.subscribe((event: AgentEvent) => {
                if (event.type === "message_update" &&
                    event.assistantMessageEvent.type === "text_delta") {
                    fullText += event.assistantMessageEvent.delta;
                } else if (event.type === "agent_end") {
                    resolve(fullText || "(no response)");
                }
            });

            this.agent.prompt(message, { toolChoice: undefined })
                .catch((err: Error) => reject(err.message));
        });
    }

    /**
     * Streaming chat — sends events through callbacks.
     */
    async chatStream(
        message: string,
        history?: Array<{ role: string; content: string }>,
        systemPrompt?: string,
        callbacks?: AgentCallbacks,
    ): Promise<void> {
        this.currentAbortController = new AbortController();

        if (systemPrompt) {
            this.agent.setSystemPrompt([systemPrompt]);
        }

        if (!callbacks) {
            // Just run silently
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
                                callbacks.onToken(delta);
                            }
                            break;
                        case "tool_execution_start":
                            callbacks.onToolStart(event.toolName);
                            break;
                        case "tool_execution_end":
                            callbacks.onToolEnd(
                                event.toolName,
                                JSON.stringify(event.result),
                            );
                            break;
                        case "agent_end":
                            callbacks.onDone(fullText);
                            unsubscribe();
                            resolve();
                            break;
                    }
                } catch (err) {
                    callbacks.onError(String(err));
                    unsubscribe();
                    reject(err);
                }
            });

            this.agent.prompt(message, { toolChoice: undefined })
                .catch((err: Error) => {
                    callbacks.onError(err.message);
                    reject(err.message);
                });
        });
    }

    /**
     * Set custom tools for the agent to use.
     */
    setTools(tools: AgentTool[]): void {
        this.agent.setTools(tools);
    }

    /**
     * Abort the current operation.
     */
    abort(): void {
        this.currentAbortController?.abort();
        this.currentAbortController = null;
    }
}

/**
 * Companion Agent Sidecar — main entry point.
 *
 * Reads JSON-RPC requests from stdin, processes them via pi-agent-core,
 * and writes NDJSON responses to stdout.
 */
import { createInterface } from "node:readline";
import { AgentManager } from "./agent";
import { encodeResult, encodeEvent, encodeError, parseRequest } from "./protocol";
import type { ChatParams, SetModelParams } from "./protocol";
import { transcribeAudio, synthesizeAudio } from "./audio";
import { transcribeLocal, synthesizeLocal } from "./audio";

let agentManager: AgentManager;
let pendingCount = 0;
let stdinEnded = false;

function send(id: string, type: "result" | "event" | "error", body: unknown): void {
    if (type === "result") {
        process.stdout.write(encodeResult(id, body));
    } else if (type === "event") {
        const evt = body as { event: string; data: unknown };
        process.stdout.write(encodeEvent(id, evt.event, evt.data));
    } else {
        const err = body as { message: string };
        process.stdout.write(encodeError(id, err.message));
    }
}

function log(message: string): void {
    process.stderr.write(`[sidecar] ${message}\n`);
}

function maybeExit(): void {
    if (stdinEnded && pendingCount === 0) {
        log("Sidecar shutting down (stdin closed, no pending requests)");
        process.exit(0);
    }
}

async function handleRequest(req: { id: string; method: string; params?: Record<string, unknown> }): Promise<void> {
    pendingCount++;
    const { id, method, params } = req;

    try {
        switch (method) {
            case "ping": {
                send(id, "result", { ok: true, model: agentManager.getModelInfo() });
                break;
            }

            case "chat": {
                const p = (params ?? {}) as ChatParams;
                const result = await agentManager.chat(p.message, p.history, p.system_prompt);
                send(id, "result", result);
                break;
            }

            case "get_history": {
                send(id, "result", { history: agentManager.getHistory() });
                break;
            }

            case "transcribe_audio": {
                const p = (params ?? {}) as { audio?: number[]; api_key?: string; base_url?: string; provider?: string };
                if (!p.audio || p.audio.length === 0) {
                    send(id, "error", { message: "missing audio data" });
                    break;
                }
                try {
                    const cfg = agentManager.getCompanionConfig();
                    const prov = p.provider || cfg.asr_provider;
                    let text: string;
                    if (prov === "local_funasr") {
                        text = await transcribeLocal(p.audio);
                    } else {
                        const apiKey = p.api_key || agentManager.getApiKeyValue();
                        text = await transcribeAudio(p.audio, apiKey, p.base_url);
                    }
                    send(id, "result", { text });
                } catch (err: any) {
                    send(id, "error", { message: `ASR: ${err.message}` });
                }
                break;
            }

            case "synthesize_audio": {
                const p = (params ?? {}) as { text?: string; voice?: string; api_key?: string; base_url?: string; provider?: string };
                if (!p.text) {
                    send(id, "error", { message: "missing text" });
                    break;
                }
                try {
                    const cfg = agentManager.getCompanionConfig();
                    const prov = p.provider || cfg.tts_provider;
                    let pcm: number[];
                    if (prov === "local_cosyvoice") {
                        pcm = await synthesizeLocal(p.text, p.voice || "default");
                    } else {
                        const apiKey = p.api_key || agentManager.getApiKeyValue();
                        pcm = await synthesizeAudio(p.text, p.voice || "茉莉", apiKey, p.base_url);
                    }
                    send(id, "result", { pcm });
                } catch (err: any) {
                    send(id, "error", { message: `TTS: ${err.message}` });
                }
                break;
            }

            case "chat_stream": {
                const p = (params ?? {}) as ChatParams;
                await agentManager.chatStream(p.message, p.history, p.system_prompt, {
                    onToken: (token) => send(id, "event", { event: "token", data: { token } }),
                    onToolStart: (name) => send(id, "event", { event: "tool_start", data: { name } }),
                    onToolEnd: (name, result) => send(id, "event", { event: "tool_end", data: { name, result } }),
                    onDone: (text) => send(id, "event", { event: "done", data: { text } }),
                    onError: (message) => send(id, "error", { message }),
                });
                break;
            }

            case "set_model": {
                const p = (params ?? {}) as SetModelParams;
                const ok = agentManager.setModel(p.provider, p.model_id, p.base_url, p.api_key);
                send(id, "result", { ok });
                break;
            }

            case "get_config": {
                send(id, "result", agentManager.getCompanionConfig());
                break;
            }

            case "update_config": {
                const p = (params ?? {}) as Record<string, unknown>;
                const updated = agentManager.updateCompanionConfig(p as any);
                send(id, "result", updated);
                break;
            }

            case "clear_history": {
                agentManager.clearHistory();
                send(id, "result", { ok: true });
                break;
            }

            case "list_character_presets": {
                send(id, "result", { presets: agentManager.listCharacterPresets() });
                break;
            }

            case "load_character_preset": {
                const p = (params ?? {}) as { file?: string };
                if (!p.file) { send(id, "error", { message: "missing file" }); break; }
                const cfg = agentManager.loadCharacterPreset(p.file);
                if (!cfg) { send(id, "error", { message: "preset not found" }); break; }
                send(id, "result", cfg);
                break;
            }

            case "agent_action": {
                // B1d: unified event-based action routing
                const p = (params ?? {}) as { type?: string; payload?: Record<string, unknown> };
                switch (p.type) {
                    case "chat": {
                        const result = await agentManager.chat(
                            (p.payload?.message as string) || "",
                            (p.payload?.history as any) || [],
                            p.payload?.system_prompt as string | undefined,
                        );
                        send(id, "result", result);
                        break;
                    }
                    case "get_history": {
                        send(id, "result", { history: agentManager.getHistory() });
                        break;
                    }
                    case "clear_history": {
                        agentManager.clearHistory();
                        send(id, "result", { ok: true });
                        break;
                    }
                    case "get_config": {
                        send(id, "result", agentManager.getCompanionConfig());
                        break;
                    }
                    case "update_config": {
                        const updated = agentManager.updateCompanionConfig(p.payload as any);
                        send(id, "result", updated);
                        break;
                    }
                    // ASR/TTS/browse: handled by Rust for now (respond with forwarded marker)
                    case "transcribe_audio": {
                        if (!p.payload?.audio) { send(id, "error", { message: "missing audio" }); break; }
                        try {
                            const apiKey = (p.payload.api_key as string) || agentManager.getApiKeyValue();
                            const text = await transcribeAudio(p.payload.audio as number[], apiKey, p.payload.base_url as string);
                            send(id, "result", { text });
                        } catch (err: any) {
                            send(id, "error", { message: `ASR: ${err.message}` });
                        }
                        break;
                    }
                    case "synthesize_audio": {
                        if (!p.payload?.text) { send(id, "error", { message: "missing text" }); break; }
                        try {
                            const apiKey = (p.payload.api_key as string) || agentManager.getApiKeyValue();
                            const pcm = await synthesizeAudio(p.payload.text as string, (p.payload.voice as string) || "茉莉", apiKey, p.payload.base_url as string);
                            send(id, "result", { pcm });
                        } catch (err: any) {
                            send(id, "error", { message: `TTS: ${err.message}` });
                        }
                        break;
                    }
                    case "browse_screenshot": {
                        send(id, "result", { forwarded: true, type: p.type, payload: p.payload });
                        break;
                    }
                    default: {
                        send(id, "error", { message: `Unknown agent action: ${p.type}` });
                        break;
                    }
                }
                break;
            }

            default: {
                send(id, "error", { message: `Unknown method: ${method}` });
                break;
            }
        }
    } catch (err) {
        log(`Error handling ${method}: ${err}`);
        send(id, "error", { message: String(err) });
    } finally {
        pendingCount--;
        maybeExit();
    }
}

async function main(): Promise<void> {
    log("Sidecar starting...");

    try {
        agentManager = new AgentManager();
        log(`Initialized with model: ${JSON.stringify(agentManager.getModelInfo())}`);
    } catch (err) {
        log(`Failed to initialize agent: ${err}`);
        process.exit(1);
    }

    const rl = createInterface({ input: process.stdin, terminal: false });

    for await (const line of rl) {
        const trimmed = line.trim();
        if (!trimmed) continue;

        const req = parseRequest(trimmed);
        if (!req) {
            log(`Invalid request: ${trimmed.slice(0, 200)}`);
            process.stdout.write(encodeError("", "Invalid JSON-RPC request"));
            continue;
        }

        // Fire and forget — async handlers increment pendingCount
        handleRequest(req).catch((err) => log(`Unhandled error: ${err}`));
    }

    // stdin ended
    stdinEnded = true;
    log("stdin closed, waiting for pending requests to complete...");
    maybeExit();
}

main().catch((err) => {
    log(`Fatal error: ${err}`);
    process.exit(1);
});

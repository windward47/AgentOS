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
                const text = await agentManager.chat(p.message, p.history, p.system_prompt);
                send(id, "result", { text });
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

            case "register_tools": {
                const p = (params ?? {}) as { tools?: Array<Record<string, unknown>>; sandbox_root?: string };
                if (p.tools) {
                    agentManager.registerToolsFromDefs(p.tools as any, p.sandbox_root || ".");
                }
                send(id, "result", { ok: true });
                break;
            }

            case "clear_history": {
                agentManager.clearHistory();
                send(id, "result", { ok: true });
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

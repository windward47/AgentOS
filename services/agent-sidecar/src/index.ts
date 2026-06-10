/**
 * Companion Agent Sidecar — main entry point.
 *
 * Reads JSON-RPC requests from stdin, processes them via pi-agent-core,
 * and writes NDJSON responses to stdout.
 *
 * Protocol:
 *   Request:  {"id":"1","method":"chat_stream","params":{"message":"hello"}}
 *   Events:   {"id":"1","type":"event","event":"token","data":{"token":"Hel"}}
 *             {"id":"1","type":"event","event":"done","data":{"text":"Hello!"}}
 *   Error:    {"id":"1","type":"error","error":{"message":"..."}}
 */
import { createInterface } from "node:readline";
import { AgentManager } from "./agent";
import type { AgentTool } from "@oh-my-pi/pi-agent-core";
import {
    type ChatParams,
    type JsonRpcRequest,
    type SetModelParams,
    encodeResult,
    encodeEvent,
    encodeError,
    parseRequest,
} from "./protocol";

let agentManager: AgentManager;

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
    // stderr is for logs only — parent reads stdout for protocol
    process.stderr.write(`[sidecar] ${message}\n`);
}

async function handleRequest(req: JsonRpcRequest): Promise<void> {
    const { id, method, params } = req;

    try {
        switch (method) {
            case "ping": {
                send(id, "result", {
                    ok: true,
                    model: agentManager.getModelInfo(),
                });
                break;
            }

            case "chat": {
                const p = (params ?? {}) as ChatParams;
                const text = await agentManager.chat(
                    p.message,
                    p.history,
                    p.system_prompt,
                );
                send(id, "result", { text });
                break;
            }

            case "chat_stream": {
                const p = (params ?? {}) as ChatParams;
                await agentManager.chatStream(
                    p.message,
                    p.history,
                    p.system_prompt,
                    {
                        onToken: (token) => {
                            send(id, "event", { event: "token", data: { token } });
                        },
                        onToolStart: (name) => {
                            send(id, "event", { event: "tool_start", data: { name } });
                        },
                        onToolEnd: (name, result) => {
                            send(id, "event", { event: "tool_end", data: { name, result } });
                        },
                        onDone: (text) => {
                            send(id, "event", { event: "done", data: { text } });
                        },
                        onError: (message) => {
                            send(id, "error", { message });
                        },
                    },
                );
                break;
            }

            case "set_model": {
                const p = (params ?? {}) as SetModelParams;
                const ok = agentManager.setModel(
                    p.provider,
                    p.model_id,
                    p.base_url,
                    p.api_key,
                );
                send(id, "result", { ok });
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

    // Read JSON-RPC requests from stdin
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

        // Handle asynchronously but don't await — allows concurrent requests
        handleRequest(req).catch((err) => {
            log(`Unhandled error: ${err}`);
        });
    }

    log("Sidecar shutting down (stdin closed)");
    process.exit(0);
}

main().catch((err) => {
    log(`Fatal error: ${err}`);
    process.exit(1);
});

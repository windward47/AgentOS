/**
 * Companion Agent Sidecar — HTTP server.
 *
 * POST /rpc          — JSON-RPC request → JSON response
 * POST /chat_stream  — JSON-RPC request → NDJSON stream (token by token)
 */
import { AgentManager } from "./agent";
import { encodeResult, encodeEvent, encodeError } from "./protocol";
import { transcribeAudio, synthesizeAudio } from "./audio";

const PORT = parseInt(process.env.COMPANION_SIDECAR_PORT || "9876");
let agentManager: AgentManager;

function log(message: string): void { process.stderr.write(`[sidecar] ${message}\n`); }

async function handleRpc(req: { id: string; method: string; params?: Record<string, unknown> }): Promise<unknown> {
    const { method, params } = req;
    switch (method) {
        case "ping": return { ok: true, model: agentManager.getModelInfo() };
        case "chat": {
            const p = (params ?? {}) as ChatParams;
            return await agentManager.chat(p.message, p.history, p.system_prompt);
        }
        case "get_history": return { history: agentManager.getHistory() };
        case "get_config": return agentManager.getCompanionConfig();
        case "update_config": return agentManager.updateCompanionConfig((params ?? {}) as any);
        case "clear_history": agentManager.clearHistory(); return { ok: true };
        case "list_conversations": return { conversations: agentManager.listConversations() };
        case "get_current_conversation": return { id: agentManager.getCurrentConversationId(), messages: agentManager.getHistory() };
        case "create_conversation": {
            const meta = agentManager.createConversation((params as any)?.title);
            return { conversation: meta, id: meta.id };
        }
        case "switch_conversation": {
            const p = params as any;
            if (!p?.id) throw new Error("missing id");
            const r = agentManager.switchConversation(p.id);
            if (!r) throw new Error("not found");
            return { conversation: r.meta, messages: r.messages };
        }
        case "delete_conversation": {
            const p = params as any;
            if (!p?.id) throw new Error("missing id");
            return { ok: agentManager.deleteConversation(p.id), currentId: agentManager.getCurrentConversationId() };
        }
        case "rename_conversation": {
            const p = params as any;
            if (!p?.id || !p?.title) throw new Error("missing id/title");
            return { ok: agentManager.renameConversation(p.id, p.title) };
        }
        case "list_memories": return { memories: agentManager.listMemories() };
        case "forget_memory": {
            const p = params as any;
            if (!p?.id) throw new Error("missing id");
            return { ok: agentManager.forgetMemory(p.id) };
        }
        case "transcribe_audio": {
            const p = params as any;
            if (!p?.audio?.length) throw new Error("missing audio");
            return { text: await transcribeAudio(p.audio, p.api_key || agentManager.getApiKeyValue(), p.base_url) };
        }
        case "synthesize_audio": {
            const p = params as any;
            if (!p?.text) throw new Error("missing text");
            return { pcm: await synthesizeAudio(p.text, p.voice || "茉莉", p.api_key || agentManager.getApiKeyValue(), p.base_url) };
        }
        default: throw new Error(`Unknown method: ${method}`);
    }
}

async function main(): Promise<void> {
    log("Sidecar starting (HTTP)...");
    agentManager = new AgentManager();
    log(`Model: ${JSON.stringify(agentManager.getModelInfo())}`);

    process.stdout.write(`${PORT}\n`);

    Bun.serve({
        port: PORT,
        hostname: "127.0.0.1",
        async fetch(req) {
            const url = new URL(req.url);
            if (req.method !== "POST") return new Response("POST only", { status: 405 });

            try {
                const body = await req.json();
                if (!body.id || !body.method) return new Response('{"error":"invalid rpc"}', { status: 400, headers: { "Content-Type": "application/json" } });

                if (url.pathname === "/chat_stream") {
                    // SSE (Server-Sent Events) response
                    const msg = (body.message ?? body.params?.message ?? "").toString();
                    const hist = (body.history ?? body.params?.history ?? []) as any[];
                    let closed = false;
                    const { readable, writable } = new TransformStream();
                    const writer = writable.getWriter();
                    const encoder = new TextEncoder();
                    const write = (data: string) => { if (!closed) writer.write(encoder.encode(`data: ${data}\n\n`)); };

                    agentManager.chatStream(msg, hist, undefined, {
                        onToken: (t) => write(encodeEvent(body.id, "token", { token: t })),
                        onToolStart: (n) => write(encodeEvent(body.id, "tool_start", { name: n })),
                        onToolEnd: (n, r) => write(encodeEvent(body.id, "tool_end", { name: n, result: r })),
                        onDone: (t) => { write(encodeEvent(body.id, "done", { text: t })); closed = true; writer.close(); },
                        onError: (m) => { write(encodeError(body.id, m)); closed = true; writer.close(); },
                    }).catch((err: any) => {
                        if (!closed) { write(encodeError(body.id, err?.message || String(err))); closed = true; writer.close(); }
                    });

                    return new Response(readable, { headers: { "Content-Type": "text/event-stream" } });
                }

                // Regular RPC
                const result = await handleRpc(body);
                return new Response(encodeResult(body.id, result), { headers: { "Content-Type": "application/json" } });
            } catch (err: any) {
                return new Response(encodeError("", err?.message || String(err)), { status: 500, headers: { "Content-Type": "application/json" } });
            }
        },
    });
    log(`Listening on http://127.0.0.1:${PORT}`);
}

main().catch((err) => { log(`Fatal: ${err}`); process.exit(1); });

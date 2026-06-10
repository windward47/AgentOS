/**
 * JSON-RPC protocol types for Companion ↔ Bun sidecar communication.
 *
 * Protocol: NDJSON over stdin/stdout.
 * - Parent (Tauri Rust) writes JSON-RPC requests to sidecar's stdin
 * - Sidecar writes NDJSON responses/events to stdout
 * - stderr is reserved for logging/debugging
 */

// ─── Request ─────────────────────────────────────────────────────
export interface JsonRpcRequest {
    id: string;
    method: "chat" | "chat_stream" | "set_model" | "clear_history" | "ping";
    params?: Record<string, unknown>;
}

export interface ChatParams {
    message: string;
    history?: Array<{ role: "user" | "assistant" | "system"; content: string }>;
    system_prompt?: string;
}

export interface SetModelParams {
    provider: string;
    model_id: string;
    base_url?: string;
    api_key?: string;
}

// ─── Response ────────────────────────────────────────────────────

export interface JsonRpcResponse {
    id: string;
    type: "result" | "event" | "error";
}

export interface JsonRpcResult extends JsonRpcResponse {
    type: "result";
    result: unknown;
}

export interface JsonRpcEvent extends JsonRpcResponse {
    type: "event";
    event: string;
    data: unknown;
}

export interface JsonRpcError extends JsonRpcResponse {
    type: "error";
    error: { message: string; code?: number };
}

export type JsonRpcMessage = JsonRpcRequest | JsonRpcResult | JsonRpcEvent | JsonRpcError;

// ─── Event types (for chat_stream) ───────────────────────────────

export interface StreamTokenEvent {
    token: string;
}

export interface StreamToolStartEvent {
    name: string;
}

export interface StreamToolEndEvent {
    name: string;
    result: string;
}

export interface StreamDoneEvent {
    text: string;
}

// ─── Helpers ─────────────────────────────────────────────────────

export function encodeResult(id: string, result: unknown): string {
    return JSON.stringify({ id, type: "result", result }) + "\n";
}

export function encodeEvent(id: string, event: string, data: unknown): string {
    return JSON.stringify({ id, type: "event", event, data }) + "\n";
}

export function encodeError(id: string, message: string, code?: number): string {
    return JSON.stringify({ id, type: "error", error: { message, code } }) + "\n";
}

export function parseRequest(line: string): JsonRpcRequest | null {
    try {
        const obj = JSON.parse(line);
        if (typeof obj.id === "string" && typeof obj.method === "string") {
            return obj as JsonRpcRequest;
        }
        return null;
    } catch {
        return null;
    }
}

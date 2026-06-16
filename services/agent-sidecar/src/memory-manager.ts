/**
 * memory-manager — Mnemopi persistent memory wrapper.
 *
 * Extracted from AgentManager to keep agent.ts lean.
 */
import { Mnemopi } from "@oh-my-pi/pi-mnemopi";

export class MemoryManager {
    private memory: Mnemopi;

    constructor() {
        this.memory = new Mnemopi({ noEmbeddings: true });
    }

    /** Search for relevant memories. Returns formatted context string. */
    async recallMemories(query: string): Promise<string> {
        try {
            const results = await this.memory.recall(query, 3);
            if (results.length === 0) return "";
            const lines = results.map(r => {
                const ts = r.timestamp ? ` (${r.timestamp.slice(0, 10)})` : "";
                return `- ${r.content}${ts}`;
            });
            return `\n<memories>\nRecalled facts from past conversations:\n${lines.join("\n")}\n</memories>`;
        } catch {
            return "";
        }
    }

    /** Store a fact. Strips <memories>/<think> tags to prevent feedback loop. */
    retainMemory(content: string): void {
        try {
            // Strip memory/think tags like omp's prepareRetentionTranscript
            const clean = content
                .replace(/<memories>[\s\S]*?<\/memories>/gi, "")
                .replace(/<think>[\s\S]*?<\/think>/gi, "")
                .trim();
            if (!clean) return;
            this.memory.remember(clean, { source: "conversation" });
        } catch {}
    }

    /** List all stored memories (most recent first). */
    listMemories(): Array<{ id: string; content: string; timestamp: string }> {
        try {
            const results = this.memory.getContext(50) as any[];
            return results.map((r: any) => ({
                id: r.id || "",
                content: r.content || "",
                timestamp: r.timestamp || "",
            }));
        } catch {
            return [];
        }
    }

    /** Delete a single memory by ID. */
    forgetMemory(id: string): boolean {
        try {
            this.memory.forget(id);
            return true;
        } catch {
            return false;
        }
    }
}

/**
 * Debug the YAML config loader.
 */
import { readFileSync } from "node:fs";
import { join } from "node:os";
import { homedir } from "node:os";

const MODELS_YML = join(homedir(), ".omp", "agent", "models.yml");
const text = readFileSync(MODELS_YML, "utf-8");
console.log("--- raw yaml ---");
console.log(text);
console.log("--- raw parse ---");

// Let me parse manually to see what's happening
function parseSimpleYaml(text: string): Record<string, unknown> {
    const result: Record<string, unknown> = {};
    const lines = text.split("\n");
    const stack: Array<{ key: string; obj: Record<string, unknown>; indent: number }> = [];

    for (const rawLine of lines) {
        const commentIdx = rawLine.indexOf("  #");
        const line = commentIdx >= 0 ? rawLine.slice(0, commentIdx) : rawLine;
        if (!line.trim() || line.trim().startsWith("#")) continue;

        const indent = line.search(/\S/);
        const trimmed = line.trim();

        if (trimmed.startsWith("- ")) {
            // Array item
            continue;
        }

        const colonIdx = trimmed.indexOf(":");
        if (colonIdx === -1) continue;

        const key = trimmed.slice(0, colonIdx).trim();
        let value: string = trimmed.slice(colonIdx + 1).trim();

        while (stack.length > 0 && indent <= stack[stack.length - 1]!.indent) {
            stack.pop();
        }

        if (value === "") {
            value = "{}";
        } else {
            value = value.replace(/^["']|["']$/g, "");
        }

        if (stack.length > 0) {
            const parent = stack[stack.length - 1]!.obj;
            if (value === "{}") {
                const newObj: Record<string, unknown> = {};
                parent[key] = newObj;
                stack.push({ key, obj: newObj, indent });
            } else {
                parent[key] = value;
            }
        } else {
            if (value === "{}") {
                const newObj: Record<string, unknown> = {};
                result[key] = newObj;
                stack.push({ key, obj: newObj, indent });
            } else {
                result[key] = value;
            }
        }
    }

    return result;
}

const parsed = parseSimpleYaml(text);
console.log(JSON.stringify(parsed, null, 2));

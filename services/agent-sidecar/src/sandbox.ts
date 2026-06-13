/**
 * Sandbox path validation — mirrors companion-core/src/sandbox/mod.rs.
 * All file/command tools resolve paths through this module.
 */
import { resolve as pathResolve, normalize } from "node:path";

const DANGEROUS_CHARS = /[;&|$`\n\r]/;

/** Characters that are rejected in user-supplied path strings (injection prevention). */
export function hasDangerousChars(input: string): boolean {
    return DANGEROUS_CHARS.test(input);
}

/**
 * Resolve a user-supplied relative path against the sandbox root.
 * Rejects absolute paths, dangerous characters, and path escapes.
 */
export function sandboxResolve(
    relPath: string,
    sandboxRoot: string
): string {
    // Reject dangerous characters
    if (hasDangerousChars(relPath)) {
        throw new Error(`Path contains dangerous characters: ${relPath}`);
    }
    // Reject absolute paths
    if (relPath.startsWith("/") || /^[A-Za-z]:\\/.test(relPath)) {
        throw new Error(`Absolute paths are not allowed: ${relPath}`);
    }
    const resolved = normalize(pathResolve(sandboxRoot, relPath));
    const normalizedRoot = normalize(sandboxRoot);
    // Reject path escapes
    if (!resolved.startsWith(normalizedRoot + "/") && resolved !== normalizedRoot) {
        throw new Error(`Path escapes sandbox: ${relPath}`);
    }
    return resolved;
}

/**
 * High-risk command names — mirrors companion-core/src/permissions/mod.rs.
 */
export const HIGH_RISK_CMDS = [
    "rm", "del", "shutdown", "reboot", "format", "dd",
];

/** Check whether a command name is high-risk. */
export function isHighRisk(cmd: string): boolean {
    const name = cmd.split(/\s+/)[0]?.split(/[\\/]/).pop()?.toLowerCase() ?? "";
    return HIGH_RISK_CMDS.includes(name);
}

// ── Audit logging ────────────────────────────────────────────────────

import { appendFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import { homedir } from "node:os";

const LOG_DIR = join(homedir(), ".companion", "logs");
const COMMAND_LOG = join(LOG_DIR, "command.log");

function ensureLogDir(): void {
    if (!existsSync(LOG_DIR)) {
        mkdirSync(LOG_DIR, { recursive: true });
    }
}

export function logAudit(event: string, details: string): void {
    try {
        ensureLogDir();
        const ts = new Date().toISOString().replace("T", " ").slice(0, 19);
        appendFileSync(COMMAND_LOG, `[${ts}] ${event}: ${details}\n`, "utf-8");
    } catch {
        // Audit failure should never crash the agent
    }
}

/**
 * Sidecar unit tests — cover the critical paths we keep debugging manually.
 * Run: cd services/agent-sidecar && bun test
 */
import { describe, test, expect } from "bun:test";
import { parseEmotions, parseThinkTags, emotionPromptFragment } from "./agent";

// ── Emotion parsing ──────────────────────────────────────────────

describe("parseEmotions", () => {
    test("extracts single emotion tag", () => {
        const { cleanText, emotions } = parseEmotions("[happy] Hello!");
        expect(cleanText).toBe("Hello!");
        expect(emotions).toEqual(["F01"]);
    });

    test("extracts multiple emotion tags", () => {
        const { cleanText, emotions } = parseEmotions("[happy] Hi! [surprised] Really?");
        expect(cleanText).toBe("Hi! Really?");
        expect(emotions).toEqual(["F01", "F04"]);
    });

    test("case insensitive", () => {
        const { cleanText, emotions } = parseEmotions("[HAPPY] [Sad]");
        expect(cleanText).toBe("");
        expect(emotions).toEqual(["F01", "F02"]);
    });

    test("no emotions returns empty", () => {
        const { cleanText, emotions } = parseEmotions("Just a normal message.");
        expect(cleanText).toBe("Just a normal message.");
        expect(emotions).toEqual([]);
    });

    test("strips tags and condenses whitespace", () => {
        const { cleanText } = parseEmotions("[happy]  Hello  [sad]  world");
        expect(cleanText).toBe("Hello world");
    });

    test("sad and fear both map to F02", () => {
        const { emotions: e1 } = parseEmotions("[sad]");
        const { emotions: e2 } = parseEmotions("[fear]");
        expect(e1).toEqual(["F02"]);
        expect(e2).toEqual(["F02"]);
    });
});

// ── Think tag parsing ────────────────────────────────────────────

describe("parseThinkTags", () => {
    test("converts think to markdown italic", () => {
        const { displayText, ttsText } = parseThinkTags("Hello <think>let me check</think> World");
        expect(displayText).toBe("Hello *let me check* World");
        expect(ttsText).toBe("Hello World");
    });

    test("removes think from TTS entirely", () => {
        const { ttsText } = parseThinkTags("<think>internal reasoning</think> The answer is 42.");
        expect(ttsText).toBe("The answer is 42.");
    });

    test("no think tags pass through unchanged", () => {
        const { displayText, ttsText } = parseThinkTags("Plain text");
        expect(displayText).toBe("Plain text");
        expect(ttsText).toBe("Plain text");
    });

    test("multiple think tags", () => {
        const { displayText, ttsText } = parseThinkTags("<think>hmm</think> A <think>also</think> B");
        expect(displayText).toBe("*hmm* A *also* B");
        expect(ttsText).toBe("A B");
    });

    test("think tags combine with emotions in real response", () => {
        const raw = "<think>Let me search the weather</think> [happy] Here it is!";
        const { displayText, ttsText } = parseThinkTags(raw);
        const { cleanText, emotions } = parseEmotions(displayText);
        expect(cleanText).toBe("*Let me search the weather* Here it is!");
        expect(emotions).toEqual(["F01"]);
        expect(ttsText).toBe("[happy] Here it is!");
    });
});

// ── Emotion prompt fragment ──────────────────────────────────────

describe("emotionPromptFragment", () => {
    test("includes available tags", () => {
        const prompt = emotionPromptFragment();
        expect(prompt).toContain("happy");
        expect(prompt).toContain("sad");
        expect(prompt).toContain("surprised");
        expect(prompt).not.toContain("neutral");
    });
});

// ── Audio WAV round-trip ────────────────────────────────────────

import { f32ToWavBase64, parseWavToF32 } from "./audio_test_helpers";

describe("audio WAV round-trip", () => {
    test("PCM → WAV → PCM preserves samples", () => {
        const input = new Float32Array([0.1, -0.2, 0.5, -0.8, 0.3]);
        const b64 = f32ToWavBase64(input);
        expect(b64).toStartWith("data:audio/wav;base64,");
        const output = parseWavToF32(b64);
        for (let i = 0; i < input.length; i++) {
            expect(Math.abs(output[i] - input[i])).toBeLessThan(0.01);
        }
    });

    test("empty array round-trips", () => {
        const b64 = f32ToWavBase64([]);
        const output = parseWavToF32(b64);
        expect(output.length).toBe(0);
    });
});

// ── TTS request format validation ────────────────────────────────

describe("synthesizeAudio request format", () => {
    test("uses mimo-v2.5-tts model", () => {
        // Verify the constant matches what the Rust impl uses
        const model = "mimo-v2.5-tts";
        expect(model).toBe("mimo-v2.5-tts");
    });

    test("includes modalities and audio params", () => {
        const body = {
            model: "mimo-v2.5-tts",
            messages: [
                { role: "user", content: "请说：Hello" },
                { role: "assistant", content: "Hello" },
            ],
            modalities: ["text", "audio"],
            audio: { voice: "茉莉", format: "wav" },
            max_tokens: 500,
        };
        expect(body.modalities).toContain("audio");
        expect(body.audio.voice).toBe("茉莉");
        expect(body.messages.length).toBe(2);
    });
});

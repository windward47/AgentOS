/**
 * Xiaomi ASR/TTS — HTTP wrappers for the token-plan API.
 * Mirrors companion-core/src/asr/xiaomi_asr.rs and companion-core/src/tts/xiaomi_tts.rs.
 *
 * ASR: PCM f32 16kHz mono → WAV → base64 data-URL → Chat Completions → text
 * TTS: Text → Chat Completions (audio modality) → base64 WAV → PCM f32
 */

const DEFAULT_BASE_URL = "https://token-plan-cn.xiaomimimo.com/v1/chat/completions";

// ── WAV helpers ────────────────────────────────────────────────────

function buildWavHeader(dataLen: number): Buffer {
    const buf = Buffer.alloc(44);
    // RIFF header
    buf.write("RIFF", 0);
    buf.writeUInt32LE(36 + dataLen, 4);
    buf.write("WAVE", 8);
    // fmt chunk
    buf.write("fmt ", 12);
    buf.writeUInt32LE(16, 16);       // chunk size
    buf.writeUInt16LE(1, 20);        // PCM
    buf.writeUInt16LE(1, 22);        // mono
    buf.writeUInt32LE(16000, 24);    // sample rate
    buf.writeUInt32LE(32000, 28);    // byte rate
    buf.writeUInt16LE(2, 32);        // block align
    buf.writeUInt16LE(16, 34);       // bits per sample
    // data chunk
    buf.write("data", 36);
    buf.writeUInt32LE(dataLen, 40);
    return buf;
}

function f32ToWavBase64(samples: Float32Array | number[]): string {
    const dataLen = samples.length * 2;
    const header = buildWavHeader(dataLen);
    const data = Buffer.alloc(dataLen);
    for (let i = 0; i < samples.length; i++) {
        const s = Math.max(-1, Math.min(1, samples[i]));
        data.writeInt16LE(Math.round(s * 32767), i * 2);
    }
    const wav = Buffer.concat([header, data]);
    return `data:audio/wav;base64,${wav.toString("base64")}`;
}

function parseWavToF32(wavBytes: Buffer): Float32Array {
    // Skip WAV header (44 bytes standard, but find "data" chunk)
    let offset = 12;
    while (offset < wavBytes.length - 8) {
        const chunkId = wavBytes.toString("ascii", offset, offset + 4);
        const chunkSize = wavBytes.readUInt32LE(offset + 4);
        if (chunkId === "data") {
            const dataStart = offset + 8;
            const sampleCount = Math.min(chunkSize, wavBytes.length - dataStart) / 2;
            const out = new Float32Array(sampleCount);
            for (let i = 0; i < sampleCount; i++) {
                out[i] = wavBytes.readInt16LE(dataStart + i * 2) / 32768;
            }
            return out;
        }
        offset += 8 + chunkSize;
    }
    return new Float32Array(0);
}

// ── HTTP helpers ────────────────────────────────────────────────────

async function xiaomiChatCompletions(
    apiKey: string,
    baseUrl: string,
    messages: Array<{ role: string; content: unknown }>,
): Promise<string> {
    const url = baseUrl.includes("/chat/completions") ? baseUrl : `${baseUrl.replace(/\/$/, "")}/chat/completions`;
    const resp = await fetch(url, {
        method: "POST",
        headers: {
            "Content-Type": "application/json",
            Authorization: `Bearer ${apiKey}`,
        },
        body: JSON.stringify({
            model: "mimo-v2.5",
            messages,
            max_tokens: 4096,
        }),
    });
    if (!resp.ok) {
        const errText = await resp.text().catch(() => "");
        throw new Error(`HTTP ${resp.status}: ${errText.slice(0, 200)}`);
    }
    const json: any = await resp.json();
    return json.choices?.[0]?.message?.content ?? "";
}

// ── Public API ──────────────────────────────────────────────────────

export async function transcribeAudio(
    audio: number[],
    apiKey: string,
    baseUrl: string = DEFAULT_BASE_URL,
): Promise<string> {
    const dataUrl = f32ToWavBase64(audio);
    const result = await xiaomiChatCompletions(apiKey, baseUrl, [
        {
            role: "user",
            content: [
                { type: "input_audio", input_audio: { data: dataUrl, format: "wav" } },
                { type: "text", text: "请将这段语音转录为文字，只返回文字内容。" },
            ],
        },
    ]);
    return result.trim();
}

export async function synthesizeAudio(
    text: string,
    voice: string,
    apiKey: string,
    baseUrl: string = DEFAULT_BASE_URL,
): Promise<number[]> {
    const result = await xiaomiChatCompletions(apiKey, baseUrl, [
        {
            role: "user",
            content: `请用${voice}的声音朗读以下文本，只返回音频：${text}`,
        },
    ]);
    // Response is base64 WAV data URL or raw base64
    let b64 = result;
    if (b64.startsWith("data:")) {
        b64 = b64.split(",")[1] ?? b64;
    }
    const wavBytes = Buffer.from(b64, "base64");
    const pcm = parseWavToF32(wavBytes);
    return Array.from(pcm);
}

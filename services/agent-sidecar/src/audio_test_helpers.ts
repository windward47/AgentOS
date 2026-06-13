/**
 * Re-export internal audio.ts functions for testing.
 * Bun can import .ts files directly in tests.
 */
export { f32ToWavBase64 } from "./audio";

// parseWavToF32 takes a Buffer (from audio.ts), but in tests we get a base64 data URL.
// So we provide a wrapper that decodes the data URL first.
import { Buffer } from "buffer";

export function parseWavToF32(dataUrl: string): Float32Array {
    let b64 = dataUrl;
    if (b64.startsWith("data:")) {
        b64 = b64.split(",")[1] ?? b64;
    }
    const wavBytes = Buffer.from(b64, "base64");
    // Re-implement WAV parsing inline for test isolation
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

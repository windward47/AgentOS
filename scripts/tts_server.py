"""OpenAI-compatible TTS server using edge-tts (Microsoft Edge free TTS).
   POST /v1/audio/speech → WAV audio bytes
   Run: python tts_server.py --port 50000
"""
import argparse, io, asyncio, tempfile, os
from fastapi import FastAPI
from fastapi.responses import Response
import uvicorn

app = FastAPI()

DEFAULT_VOICE = "zh-CN-XiaoxiaoNeural"
AVAILABLE_VOICES = [
    "zh-CN-XiaoxiaoNeural", "zh-CN-YunxiNeural", "zh-CN-YunjianNeural",
    "zh-CN-XiaoyiNeural", "zh-CN-YunyangNeural", "zh-CN-XiaochenNeural",
    "zh-CN-XiaohanNeural", "zh-CN-XiaomengNeural", "zh-CN-XiaomoNeural",
    "zh-CN-XiaoqiuNeural", "zh-CN-XiaoruiNeural", "zh-CN-XiaoshuangNeural",
    "zh-CN-XiaoxuanNeural", "zh-CN-XiaoyanNeural", "zh-CN-XiaozhenNeural",
]

@app.get("/v1/models")
async def list_models():
    return {"object": "list", "data": [{"id": v, "object": "model"} for v in AVAILABLE_VOICES]}

@app.post("/v1/audio/speech")
async def synthesize(request: dict):
    import edge_tts, struct, subprocess
    text = request.get("input", "")
    voice = request.get("voice", DEFAULT_VOICE)
    if not text:
        return Response(status_code=400)

    tmp_mp3 = tempfile.NamedTemporaryFile(suffix=".mp3", delete=False)
    tmp_mp3.close()
    tmp_wav = tempfile.NamedTemporaryFile(suffix=".wav", delete=False)
    tmp_wav.close()
    try:
        communicate = edge_tts.Communicate(text, voice)
        await communicate.save(tmp_mp3.name)
        # Convert MP3 to 16kHz mono WAV via ffmpeg (or pydub fallback)
        try:
            subprocess.run(["ffmpeg", "-y", "-i", tmp_mp3.name, "-ar", "16000", "-ac", "1", "-f", "wav", tmp_wav.name],
                         check=True, capture_output=True, timeout=30)
        except (FileNotFoundError, subprocess.CalledProcessError):
            # Fallback: use wave module to read MP3 with pydub
            from pydub import AudioSegment
            audio = AudioSegment.from_mp3(tmp_mp3.name)
            audio = audio.set_frame_rate(16000).set_channels(1)
            audio.export(tmp_wav.name, format="wav")
        # Read WAV → PCM f32
        import wave
        with wave.open(tmp_wav.name, "rb") as wf:
            samples = wf.readframes(wf.getnframes())
        pcm = struct.unpack("<" + "h" * (len(samples) // 2), samples)
        f32 = [s / 32768.0 for s in pcm]
        return Response(content=struct.pack("<" + "f" * len(f32), *f32), media_type="audio/pcm")
    finally:
        for f in [tmp_mp3.name, tmp_wav.name]:
            try: os.unlink(f)
            except: pass

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=50000)
    args = parser.parse_args()
    print(f"TTS server (edge-tts) ready on http://localhost:{args.port}")
    uvicorn.run(app, host="0.0.0.0", port=args.port)

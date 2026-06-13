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
    import edge_tts, struct
    text = request.get("input", "")
    voice = request.get("voice", DEFAULT_VOICE)
    if not text:
        return Response(status_code=400)

    tmp = tempfile.NamedTemporaryFile(suffix=".mp3", delete=False)
    tmp.close()
    try:
        communicate = edge_tts.Communicate(text, voice)
        await communicate.save(tmp.name)
        # Decode MP3 to PCM f32 using torchaudio (already installed)
        import torchaudio
        waveform, sr = torchaudio.load(tmp.name)
        if sr != 16000:
            import torchaudio.functional as F
            waveform = F.resample(waveform, sr, 16000)
        # Convert to mono PCM f32
        mono = waveform.mean(dim=0) if waveform.shape[0] > 1 else waveform[0]
        pcm = mono.numpy().astype('float32').tobytes()
        return Response(content=pcm, media_type="audio/pcm")
    finally:
        os.unlink(tmp.name)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=50000)
    args = parser.parse_args()
    print(f"TTS server (edge-tts) ready on http://localhost:{args.port}")
    uvicorn.run(app, host="0.0.0.0", port=args.port)

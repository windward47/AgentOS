"""Local TTS server using edge-tts + bundled ffmpeg.
   POST /v1/audio/speech → PCM f32 bytes
   Run: python tts_server.py --port 50000
"""
import argparse, io, struct, tempfile, os, wave, subprocess
from fastapi import FastAPI
from fastapi.responses import Response
import uvicorn

FFMPEG = os.path.join(os.path.dirname(__file__), "ffmpeg.exe")
DEFAULT_VOICE = "zh-CN-XiaoxiaoNeural"

app = FastAPI()

@app.get("/v1/models")
async def list_models():
    voices = ["zh-CN-XiaoxiaoNeural","zh-CN-YunxiNeural","zh-CN-YunjianNeural","zh-CN-XiaoyiNeural"]
    return {"object": "list", "data": [{"id": v, "object": "model"} for v in voices]}

@app.post("/v1/audio/speech")
async def synthesize(request: dict):
    import edge_tts
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
        subprocess.run([FFMPEG, "-y", "-i", tmp_mp3.name, "-ar", "16000", "-ac", "1", "-f", "wav", tmp_wav.name],
                      check=True, capture_output=True, timeout=30)
        with wave.open(tmp_wav.name, "rb") as wf:
            n = wf.getnframes()
            samples = wf.readframes(n)
        pcm_i16 = struct.unpack("<" + "h" * (len(samples) // 2), samples)
        f32 = [s / 32768.0 for s in pcm_i16]
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

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=50000)
    args = parser.parse_args()
    print(f"TTS server (edge-tts) ready on http://localhost:{args.port}")
    uvicorn.run(app, host="0.0.0.0", port=args.port)

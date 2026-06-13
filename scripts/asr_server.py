"""Minimal OpenAI-compatible ASR server using FunASR SenseVoiceSmall.
   POST /v1/audio/transcriptions with multipart file=audio.wav → {"text":"..."}
   Run: python asr_server.py --port 8000
"""
import argparse, tempfile, os
from fastapi import FastAPI, File, UploadFile
import uvicorn

app = FastAPI()

@app.post("/v1/audio/transcriptions")
async def transcribe(file: UploadFile = File(...)):
    audio_bytes = await file.read()
    # Save to temp WAV file
    with tempfile.NamedTemporaryFile(suffix=".wav", delete=False) as f:
        f.write(audio_bytes)
        tmp = f.name
    try:
        result = model.generate(input=tmp, language="auto")
        text = ""
        if result and len(result) > 0:
            text = result[0].get("text", "")
        return {"text": text.strip()}
    finally:
        os.unlink(tmp)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8000)
    parser.add_argument("--device", default="cpu")
    args = parser.parse_args()
    print(f"Loading SenseVoiceSmall on {args.device}...")
    from funasr import AutoModel
    global model
    model = AutoModel(model="iic/SenseVoiceSmall", device=args.device)
    print(f"ASR server ready on http://localhost:{args.port}")
    uvicorn.run(app, host="0.0.0.0", port=args.port)

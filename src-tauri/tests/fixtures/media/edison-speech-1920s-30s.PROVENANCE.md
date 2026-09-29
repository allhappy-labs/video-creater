# Edison Speech Fixture Provenance

Fixture: `edison-speech-1920s-30s.mp4`

Purpose: Small speech-video fixture for transcription compatibility tests and optional real model E2E runs.

Upstream source: Wikimedia Commons, `File:Edison speech, 1920s.ogv`

Source URL: https://commons.wikimedia.org/wiki/File:Edison_speech,_1920s.ogv

Original media URL: https://upload.wikimedia.org/wikipedia/commons/3/3c/Edison_speech%2C_1920s.ogv

Upstream description: Thomas Edison speaking about the invention of the light bulb, late 1920s. Newsreel clip from the Motion Picture Division of the U.S. National Archives.

License/status: Public domain. The Commons page identifies the source as a U.S. National Archives newsreel / U.S. government work.

Why this derivative exists: The original OGV container has Theora video and Vorbis audio. The FluidAudio helper failed to read that audio through Foundation on macOS. This derivative keeps the first 30 seconds and transcodes to H.264/AAC MP4, which passed the real transcription E2E on macOS.

Reproduction command:

```bash
curl -L --fail -o /tmp/edison-speech-1920s.ogv \
  "https://upload.wikimedia.org/wikipedia/commons/3/3c/Edison_speech%2C_1920s.ogv"

ffmpeg -y -hide_banner -loglevel error \
  -i /tmp/edison-speech-1920s.ogv \
  -t 30 \
  -c:v libx264 -pix_fmt yuv420p \
  -c:a aac -b:a 128k \
  src-tauri/tests/fixtures/media/edison-speech-1920s-30s.mp4
```

Fixture SHA-256:

```text
05b0da3bc3f54f9a278f5890e38dd49f74c835e58c829d24f227e4f676d0b5a2
```

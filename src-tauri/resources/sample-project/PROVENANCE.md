# Bundled Edison sample provenance

The three bundled sample files are deterministic derivatives of
`src-tauri/tests/fixtures/media/edison-speech-1920s-30s.mp4`.

The upstream work is the U.S. National Archives newsreel “Edison speech,
1920s,” distributed by Wikimedia Commons as a public-domain U.S. government
work:

- Source page: <https://commons.wikimedia.org/wiki/File:Edison_speech,_1920s.ogv>
- Original media: <https://upload.wikimedia.org/wikipedia/commons/3/3c/Edison_speech%2C_1920s.ogv>
- Retained source SHA-256: `05b0da3bc3f54f9a278f5890e38dd49f74c835e58c829d24f227e4f676d0b5a2`

`media/input.mp4` is seconds 1–5, scaled and pillarboxed to
640×360 at 24 fps. `media/voiceover.m4a` is its four-second mono AAC audio.
`sample/generated/product-reveal.mp4` is seconds 5–9 with a deterministic
contrast and saturation treatment. The project identifies that alternate as
a local bundled derivative; it is not represented as provider-generated
media.

Bundled derivatives:

- `media/input.mp4`: `5b338334663eb509053ca0c2502b74db776834af4833625f5f340f6462fe3d38`
- `media/voiceover.m4a`: `4132b4c6810b18ac17da831cb1525568f26be8b6b5fbf2774fd741c30c5b6ad2`
- `sample/generated/product-reveal.mp4`: `b0adb915a1e23e58c7c70c5472925a193ff6d7119be040b3f4e8424fc7e9acf7`

# Bundled PDF font

`NotoSansCJKsc-Regular.otf` is bundled only for deterministic PDF generation. The
renderer subsets it to the glyphs used by each PDF; Office files reference a
font family but do not embed this asset.

- Upstream: `notofonts/noto-cjk/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf`
- Upstream commit: `f8d157532fbfaeda587e826d4cd5b21a49186f7c`
- Font SHA-256: `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`
- License: SIL Open Font License 1.1, preserved as `OFL-NotoSansCJK.txt`
- License SHA-256: `6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2`

Do not replace the font from a moving URL without updating the pinned commit,
both digests, visual PDF evidence, Poppler text/font checks, and the canonical
Artifact regression.

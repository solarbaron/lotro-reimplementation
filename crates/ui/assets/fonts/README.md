<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Tessera's fallback font

`NotoSans-Regular.ttf` is **Noto Sans Regular**, version 2.015, unmodified, by the Noto
Project Authors, under the SIL Open Font License 1.1 (`OFL.txt`; the font file names the
licence itself in its `name` table, IDs 13 and 14). It has no Reserved Font Name.
SHA-256 `478c558ea716033cd60c03438f628dfa75694dcf6b5f6d505a2f05fd2b4f3823`.

It is **not** a game font. `tessera-ui` (`src/text/fallback.rs`) embeds it as Tessera's own
font for text without a game layout (`docs/specs/fonts.md` §11 items 2, 4 and 5): start-up
and error screens, Tessera's settings, the plugin manager without a game layout, debug
overlays. It is rasterised on the CPU at 1 : 1 into the same one-byte coverage form as the
game's glyph atlases. Game fonts and atlases are only ever read from the user's install.

`NotoSans-Regular.ttf` and the licence text `OFL.txt` are the only files under this
directory that are not GPL-3.0-or-later: the font is OFL-1.1 (copyright The Noto Project
Authors) and stays under the OFL when Tessera is distributed (OFL §2, §4, §5); `OFL.txt` is
the licence text itself. The owner accepted bundling it in decision 0022
(`docs/decisions/0022-bundled-ui-font.md`, recorded in `docs/LEGAL.md` §11): `REUSE.toml`
carries an override annotation for both files (OFL-1.1, The Noto Project Authors), the
licence text is `LICENSES/OFL-1.1.txt`, and `tools/content-allowlist.txt` exempts exactly the
font file from the extension and font-magic checks. Do not rename or modify the font.

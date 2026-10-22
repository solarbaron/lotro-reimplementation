<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Disclaimer

## Non-affiliation (mandatory text)

This text must appear in the README, on any project website, in release notes, and in the
application's About screen. Do not shorten or reword it.

> Tessera is an independent, unofficial project. It is not affiliated with, endorsed by, or
> sponsored by Daybreak Game Company LLC, Standing Stone Games, Middle-earth Enterprises,
> LLC, Warner Bros., or the Tolkien Estate. The Lord of the Rings Online is a trademark of
> Middle-earth Enterprises, LLC used under license. Requires a legally obtained installation
> of the game; no game content is distributed.

"Tessera" is a working codename and will be replaced by the final project name.

## What this software is and is not

- Tessera is original software written from scratch. It contains **no** code, art, audio,
  text, maps or other content from the game.
- It reads data files from **your own, legally obtained installation** of the game, in place
  and read-only. It does not copy, modify, repack or redistribute them. Obtain the game only
  from its official distribution channels.
- Official-server compatibility (milestone M8,
  [decision 0034](docs/decisions/0034-official-server-compatibility.md)) is **partly done and
  needs more work**: the clean-room protocol specifications and foundational protocol modules
  in `tessera-compat` are built, while end-to-end client wiring and full gameplay message
  integration are still in progress.

## No warranty

This program is distributed under the GNU General Public License, version 3 or (at your
option) any later version, **WITHOUT ANY WARRANTY**; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for
details.

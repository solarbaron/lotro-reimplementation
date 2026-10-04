<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Reference machine for performance criteria

| | |
|---|---|
| **Status** | **Proposed, owner to confirm** (2026-10-04) |
| **Serves** | [ROADMAP](../ROADMAP.md) M3 criterion 2 (frame times), M4 criterion 4 (50 animated characters at 60 fps), M6 criterion 4 (no audio underruns), the [decision 0005](../decisions/0005-no-full-bevy.md) entity-storage benchmark; [§4 open question](../ROADMAP.md#open) |

## Proposed reference: the primary workstation

The machine on which M0–M3 were developed and on which the M3 performance evidence (frame
logs on the GTX 1070 Ti) was recorded:

| Part | Value (observed 2026-10-04) |
|---|---|
| GPU | NVIDIA GeForce GTX 1070 Ti, 8 GiB; proprietary driver 580.x, Vulkan |
| CPU | 24 hardware threads (Intel). **Owner: fill in the exact model**; `lscpu` reports a generic name on this machine |
| RAM | 32 GB |
| OS | Gentoo Linux, kernel 7.1, Wayland |
| Storage | game install on an NVMe SSD (archives in the page cache during the M3 runs) |

## Rules (proposed)

1. Performance exit criteria are **met** only when measured on the reference machine with a
   release build. Record the machine, driver, resolution, vsync setting and whether the
   archives were cached next to every number.
2. Numbers from another machine are **indicative**: record them with that machine's
   description; they can show a regression, but not close a criterion.
3. If the reference machine changes (hardware upgrade, or another machine becomes the main
   one), re-run the M3 frame-time measurement there first, so later results can be
   compared with the M3 numbers, and record the change here with a date.

> **OPEN:** owner confirmation of this machine (or another) as the reference, and of the
> rules above. Until then, M4 performance work may proceed, but criterion 4 stays open.

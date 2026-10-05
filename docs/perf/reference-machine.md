<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Reference machine for performance criteria

| | |
|---|---|
| **Status** | **Machine confirmed by the owner** (2026-10-05): the GTX 1070 Ti workstation below. Not available at the moment, so the runs it serves are pending |
| **Serves** | [ROADMAP](../ROADMAP.md) M3 criterion 2 (frame times), M4 criterion 4 (50 animated characters at 60 fps), M6 criterion 4 (no audio underruns), the [decision 0005](../decisions/0005-no-full-bevy.md) entity-storage benchmark (decided in [0008](../decisions/0008-entity-storage.md) from a relative comparison on another machine, which stands; its absolute numbers on this machine are pending); [§4 open question](../ROADMAP.md#open) |

## Reference: the primary workstation

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

**Log**

- 2026-10-05: the owner named this workstation (the GTX 1070 Ti machine) as the reference.
  It is not available at the moment. Until it is, the M4 criterion 4 crowd run and the
  absolute numbers of the decision 0008 benchmark wait for it. The owner ran Tessera on the
  RTX 2070 Super Max-Q laptop the same day and reports that it runs well. Under rule 2 that
  is indicative and does not close criterion 4.

> **OPEN:** the exact CPU model of this machine (fill in when it is next available).

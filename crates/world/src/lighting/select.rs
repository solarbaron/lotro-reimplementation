// SPDX-License-Identifier: GPL-3.0-or-later
//! Which lights an object receives (`lighting.md` §8.5, L20).

/// Most lights in the per-vertex lists, and most lights submitted in all, main light
/// included (`lighting.md` §8.5; the point-light arrays hold 8 entries, LT4).
pub const MAX_LIGHTS: usize = 8;

/// A light the selection can place: its index in the caller's light table, whether it is
/// directional and whether it has `KeyLight` on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LightRef {
    /// Index in the caller's light table.
    pub id: usize,
    /// A directional light (else a point light).
    pub directional: bool,
    /// `KeyLight`.
    pub key_light: bool,
}

/// The per-pixel lighting options of the client (`lighting.md` §8.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectionOptions {
    /// The per-pixel lighting option: key lights become per-pixel candidates.
    pub per_pixel: bool,
    /// The engine option that makes every light a per-pixel candidate.
    pub all_per_pixel: bool,
}

/// The lights one object receives (`lighting.md` §8.5).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LightSelection {
    /// Directional light 0 (per pixel).
    pub directional0: Option<usize>,
    /// Point light 0 (per pixel).
    pub point0: Option<usize>,
    /// The per-vertex point-light list, in submission order.
    pub vertex_points: Vec<usize>,
    /// The per-vertex extra directional slot (`lighting.md` §8.2; count 0 or 1).
    pub vertex_directional: Option<usize>,
    /// No main light and no directional light in [`LightSelection::directional0`]: the
    /// submission path adds a default directional light, black as shipped (`lighting.md`
    /// §8.5). **Tessera Decision** (§8.5 OPEN): it stands in for the main light, so it
    /// takes the directional light-0 slot (a per-pixel directional light from the object's
    /// list that takes that slot replaces it and clears this flag) and counts toward the
    /// [`MAX_LIGHTS`] in all.
    pub default_directional: bool,
    /// Lights left out by the limits.
    pub dropped: usize,
}

impl LightSelection {
    /// Lights submitted in all, the default directional light included.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.directional0.is_some() || self.default_directional)
            + usize::from(self.point0.is_some())
            + self.vertex_points.len()
            + usize::from(self.vertex_directional.is_some())
    }

    /// Whether nothing is submitted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn vertex_len(&self) -> usize {
        self.vertex_points.len() + usize::from(self.vertex_directional.is_some())
    }

    /// Puts `l` into the per-vertex lists, within the limits.
    fn push_vertex(&mut self, l: LightRef) {
        if self.len() >= MAX_LIGHTS || self.vertex_len() >= MAX_LIGHTS {
            self.dropped += 1;
            return;
        }
        if l.directional {
            if self.vertex_directional.is_none() {
                self.vertex_directional = Some(l.id);
            } else {
                // One extra directional slot only (§8.2).
                self.dropped += 1;
            }
        } else {
            self.vertex_points.push(l.id);
        }
    }
}

/// Selects an object's lights (`lighting.md` §8.5): first the scene's `main` light (light 0
/// when directional; the per-vertex point list when a point light); then the object's
/// `list` in order: with the per-pixel option, a key light (or every light with
/// `all_per_pixel`) is a per-pixel candidate and the first of each kind (directional,
/// point) takes the light-0 slot of that kind while it is free, later ones go to the
/// per-vertex lists; every other light goes to the per-vertex lists. At most
/// [`MAX_LIGHTS`] per-vertex and [`MAX_LIGHTS`] in all; further lights are dropped and
/// counted. Without a main light the default directional light is flagged; it holds the
/// directional light-0 slot and one of the [`MAX_LIGHTS`] until a per-pixel directional
/// light takes that slot (**Tessera Decision**, see
/// [`LightSelection::default_directional`]).
#[must_use]
pub fn select_lights(
    main: Option<LightRef>,
    list: &[LightRef],
    opts: SelectionOptions,
) -> LightSelection {
    let mut s = LightSelection::default();
    match main {
        Some(m) if m.directional => s.directional0 = Some(m.id),
        Some(m) => s.push_vertex(m),
        None => s.default_directional = true,
    }
    for &l in list {
        if s.len() >= MAX_LIGHTS {
            s.dropped += 1;
            continue;
        }
        let candidate = opts.per_pixel && (l.key_light || opts.all_per_pixel);
        if candidate {
            let slot = if l.directional {
                &mut s.directional0
            } else {
                &mut s.point0
            };
            if slot.is_none() {
                *slot = Some(l.id);
                if l.directional {
                    // The real light 0 replaces the default one in its slot.
                    s.default_directional = false;
                }
                continue;
            }
        }
        s.push_vertex(l);
    }
    s
}

/// A light's reach for the list rule: position and range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightReach {
    /// Index in the caller's light table.
    pub id: usize,
    /// Position, the object's frame.
    pub position: [f32; 3],
    /// Range *R*.
    pub range: f32,
}

/// The object's light list (`lighting.md` §8.5 OPEN; **Tessera Decision** there): the
/// lights whose range sphere touches the object's bounding sphere (`centre`, `radius`),
/// nearest first (ties by id). Lights with non-finite values are skipped.
#[must_use]
pub fn object_light_list(centre: [f32; 3], radius: f32, lights: &[LightReach]) -> Vec<usize> {
    if !(centre.iter().all(|c| c.is_finite()) && radius.is_finite()) {
        return Vec::new();
    }
    let mut near: Vec<(f32, usize)> = lights
        .iter()
        .filter_map(|l| {
            if !(l.position.iter().all(|c| c.is_finite()) && l.range.is_finite()) {
                return None;
            }
            let d2: f32 = (0..3).map(|i| (l.position[i] - centre[i]).powi(2)).sum();
            let reach = l.range.max(0.0) + radius.max(0.0);
            (d2 <= reach * reach).then_some((d2, l.id))
        })
        .collect();
    near.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    near.into_iter().map(|(_, id)| id).collect()
}

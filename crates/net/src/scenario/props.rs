// SPDX-License-Identifier: GPL-3.0-or-later
//! Property values and looks (`scenario-feed.md` §5.8, revision 4): parsing of
//! `PROPERTY=VALUE` tokens and `look` declarations, the value check against a kind, and
//! resolution item 7 of §8.6 (property IDs, kinds, value labels, canonical looks, notes).

use super::ast::{Element, LookDecl, PValue, PropKey, PropSite};
use super::error::{Limit, ScenarioErrorKind as K};
use super::lex::{self, Token, is_label_shape};
use super::parse::{Binding, Parser, R};
use super::resolve::ResolveNote;
use super::select::{LabelContext, ResolvedRecord, SelectorResolver};
use crate::model::{
    DataId, Look, LookEntry, MAX_LOOK_BYTES, MAX_LOOK_ENTRIES, MAX_PROPERTY_STRING_LEN,
    PropertyKind, PropertyValue, is_look_string,
};

/// Most look bindings (§11.1).
pub const MAX_LOOKS: usize = 64;
/// Most property values per look (§5.8).
pub const MAX_LOOK_VALUES: usize = MAX_LOOK_ENTRIES;
/// Largest absolute value of a `float` property value (§5.8).
pub const MAX_PROPERTY_FLOAT: f64 = 1.0e9;

/// What a value check found (§5.8 "Checks").
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Checked {
    /// A complete value.
    Value(PropertyValue),
    /// `none`: clears the property.
    Clear,
    /// An enum or bitfield value written with labels, resolved through the property's
    /// mapper (`enum_value` with [`LabelContext::PropertyKey`]).
    Labels(PropertyKind, Vec<String>),
    /// A `did` value written as an element.
    Element(Element),
}

/// Why a value does not fit a kind: the form (`BadValue`) or the range (`OutOfRange`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Misfit {
    Form,
    Range,
}

impl Misfit {
    fn kind(self) -> K {
        match self {
            Self::Form => K::BadValue,
            Self::Range => K::OutOfRange,
        }
    }
}

/// Checks a written value against `kind` (§5.8 table), with the look rules when
/// `in_look`.
pub(crate) fn check(kind: PropertyKind, v: &PValue, in_look: bool) -> Result<Checked, Misfit> {
    use PropertyKind as P;
    if matches!(v, PValue::None) {
        return if in_look {
            Err(Misfit::Form)
        } else {
            Ok(Checked::Clear)
        };
    }
    let int = |v: &PValue| match v {
        PValue::Int(n, _) => Some(*n),
        _ => None,
    };
    Ok(Checked::Value(match kind {
        P::Bool => match v {
            PValue::Bool(b) => PropertyValue::Bool(*b),
            _ => return Err(Misfit::Form),
        },
        P::Int => {
            let n = int(v).ok_or(Misfit::Form)?;
            PropertyValue::Int(i32::try_from(n).map_err(|_| Misfit::Range)?)
        }
        P::Int64 => {
            let n = int(v).ok_or(Misfit::Form)?;
            PropertyValue::Int64(i64::try_from(n).map_err(|_| Misfit::Range)?)
        }
        P::Float => {
            #[allow(clippy::cast_precision_loss)] // nearest f64, as §4.2 converts
            let d = match v {
                PValue::Int(n, _) => *n as f64,
                PValue::Decimal(d) => *d,
                _ => return Err(Misfit::Form),
            };
            if !d.is_finite() || d.abs() > MAX_PROPERTY_FLOAT {
                return Err(Misfit::Range);
            }
            #[allow(clippy::cast_possible_truncation)] // nearest f32 by definition
            let f = d as f32;
            PropertyValue::Float(f)
        }
        P::Enum => match v {
            PValue::Int(n, _) => PropertyValue::Enum(u32::try_from(*n).map_err(|_| Misfit::Range)?),
            PValue::Labels(l) if l.len() == 1 => return Ok(Checked::Labels(kind, l.clone())),
            _ => return Err(Misfit::Form),
        },
        P::Bitfield32 => match v {
            PValue::Int(n, _) => {
                PropertyValue::Bitfield32(u32::try_from(*n).map_err(|_| Misfit::Range)?)
            }
            PValue::Labels(l) => return Ok(Checked::Labels(kind, l.clone())),
            _ => return Err(Misfit::Form),
        },
        P::Bitfield64 => match v {
            PValue::Int(n, _) => {
                PropertyValue::Bitfield64(u64::try_from(*n).map_err(|_| Misfit::Range)?)
            }
            PValue::Labels(l) => return Ok(Checked::Labels(kind, l.clone())),
            _ => return Err(Misfit::Form),
        },
        P::Did => match v {
            PValue::Element(e) => return Ok(Checked::Element(*e)),
            PValue::Int(n, true) => {
                PropertyValue::Did(DataId(u32::try_from(*n).map_err(|_| Misfit::Range)?))
            }
            _ => return Err(Misfit::Form),
        },
        P::String => match v {
            PValue::Str(s) => {
                if s.len() > MAX_PROPERTY_STRING_LEN || (in_look && !is_look_string(s)) {
                    return Err(Misfit::Form);
                }
                PropertyValue::String(s.clone())
            }
            _ => return Err(Misfit::Form),
        },
        P::Instance => {
            if in_look {
                return Err(Misfit::Form);
            }
            let n = int(v).ok_or(Misfit::Form)?;
            PropertyValue::Instance(u64::try_from(n).map_err(|_| Misfit::Range)?)
        }
        P::Colour => match v {
            PValue::Colour(c) => PropertyValue::Colour(*c),
            _ => return Err(Misfit::Form),
        },
        _ => return Err(Misfit::Form),
    }))
}

impl Parser {
    /// A property value token `@Label=VALUE` or `pid:N:KIND=VALUE` (§5.8); returns its
    /// index in [`Program::props`](super::ast::Program::props).
    pub(crate) fn prop_value(&mut self, tok: &Token, in_look: bool) -> R<u32> {
        let col = tok.col;
        let Some((key, value)) = tok.split_eq() else {
            return Err(self.e(K::UnexpectedToken, col));
        };
        let key = if key.starts_with('@') {
            PropKey::Label(self.label_text(key, col)?)
        } else if let Some(rest) = key.strip_prefix("pid:") {
            let (n, kind) = rest
                .rsplit_once(':')
                .ok_or_else(|| self.e(K::UnexpectedToken, col))?;
            let kind =
                PropertyKind::from_keyword(kind).ok_or_else(|| self.e(K::UnexpectedToken, col))?;
            let n = lex::int(n, true).map_err(|k| self.e(k, col))?;
            if !(1..=i128::from(u32::MAX)).contains(&n) {
                return Err(self.e(K::OutOfRange, col));
            }
            PropKey::Pid(u32::try_from(n).unwrap_or(u32::MAX), kind)
        } else {
            return Err(self.e(K::UnexpectedToken, col));
        };
        let value = self.pvalue(&value, col)?;
        if let PropKey::Pid(_, kind) = &key {
            check(*kind, &value, in_look).map_err(|m| self.e(m.kind(), col))?;
        }
        let i = u32::try_from(self.prog.props.len()).unwrap_or(u32::MAX);
        self.prog.props.push(PropSite {
            line: self.line,
            col,
            key,
            value,
            in_look,
        });
        Ok(i)
    }

    /// The value part of a property value (§4.2 "Property value", §5.8 forms).
    fn pvalue(&self, v: &Token, col: u32) -> R<PValue> {
        if let Some(s) = v.string() {
            if s.len() > MAX_PROPERTY_STRING_LEN {
                return Err(self.e(K::TooLong, col));
            }
            return Ok(PValue::Str(s.to_owned()));
        }
        let s = v.raw().ok_or_else(|| self.e(K::UnexpectedToken, col))?;
        Ok(match s {
            "yes" => PValue::Bool(true),
            "no" => PValue::Bool(false),
            "none" => PValue::None,
            _ if s.starts_with("rgba:") => {
                let parts: Vec<&str> = s["rgba:".len()..].split(',').collect();
                if parts.len() != 4 {
                    return Err(self.e(K::BadNumber, col));
                }
                let mut c = [0u8; 4];
                for (o, p) in c.iter_mut().zip(&parts) {
                    let n = lex::int(p, false).map_err(|k| self.e(k, col))?;
                    *o = u8::try_from(n).map_err(|_| self.e(K::OutOfRange, col))?;
                }
                PValue::Colour(c)
            }
            _ if s.starts_with('@') => {
                let mut labels = Vec::new();
                for l in s.split('|') {
                    if !is_label_shape(l) {
                        return Err(self.e(K::UnexpectedToken, col));
                    }
                    labels.push(self.label_text(l, col)?);
                }
                PValue::Labels(labels)
            }
            _ if s.starts_with('$') => PValue::Element(self.element_text(s, col)?),
            _ if s.starts_with(|c: char| c.is_ascii_digit() || c == '-') => {
                if s.contains('.') {
                    PValue::Decimal(lex::decimal(s, K::BadNumber).map_err(|k| self.e(k, col))?)
                } else {
                    let hex = s.starts_with("0x");
                    PValue::Int(lex::int(s, true).map_err(|k| self.e(k, col))?, hex)
                }
            }
            _ => return Err(self.e(K::UnexpectedToken, col)),
        })
    }

    /// `look $l PROPVAL { PROPVAL }` (§5.8).
    pub(crate) fn decl_look(&mut self, tokens: &[Token]) -> R<()> {
        let col = tokens[0].col;
        if self.prog.looks.len() >= MAX_LOOKS {
            return Err(self.e(K::LimitExceeded(Limit::Looks), col));
        }
        let b = tokens
            .get(1)
            .ok_or_else(|| self.e(K::MissingArgument, col))?;
        let name = self.binding_name(b)?;
        if self.bindings.contains_key(&name) {
            return Err(self.e(K::DuplicateBinding, b.col));
        }
        if tokens.len() < 3 {
            return Err(self.e(K::MissingArgument, col));
        }
        let mut props = Vec::new();
        for (i, t) in tokens[2..].iter().enumerate() {
            if i >= MAX_LOOK_VALUES {
                return Err(self.e(K::LimitExceeded(Limit::Look), t.col));
            }
            props.push(self.prop_value(t, true)?);
        }
        let index = u32::try_from(self.prog.looks.len()).unwrap_or(u32::MAX);
        self.bindings.insert(name.clone(), Binding::Look(index));
        self.prog.looks.push(LookDecl {
            name,
            line: self.line,
            props,
        });
        Ok(())
    }

    /// The value of `look=`: a look binding, or `asked` when `asked` is allowed.
    pub(crate) fn look_arg(&self, tok: &Token, col: u32, asked: bool) -> R<super::ast::LookArg> {
        if asked && tok.is("asked") {
            return Ok(super::ast::LookArg::Asked);
        }
        match self.lookup(tok, col)? {
            Binding::Look(i) => Ok(super::ast::LookArg::Binding(i)),
            _ => Err(self.e(K::WrongBindingKind, col)),
        }
    }
}

/// The written form of a property for notes: `@Name`, or `pid:0x` and 8 upper-case
/// hexadecimal digits (§10.2).
fn written(key: &PropKey) -> String {
    match key {
        PropKey::Label(l) => l.clone(),
        PropKey::Pid(n, _) => format!("pid:0x{n:08X}"),
    }
}

/// A resolved property value: the property ID and the value (`None`: clear); `None` when
/// the value was dropped (§5.8).
pub(crate) type ResolvedProp = Option<(u32, Option<PropertyValue>)>;

/// A note with its sort position (line, column, order) for §8.6's note order.
pub(crate) type LateNote = (u32, u32, usize, ResolveNote);

/// Resolution item 7 of §8.6: every property value, then every look made canonical.
/// Returns the resolved values (one per site) and looks (one per declaration); appends
/// notes to `late`.
pub(crate) fn resolve_props(
    props: &[PropSite],
    looks: &[LookDecl],
    bindings: &[Vec<ResolvedRecord>],
    resolver: &mut dyn SelectorResolver,
    install: bool,
    late: &mut Vec<LateNote>,
) -> (Vec<ResolvedProp>, Vec<Option<Look>>) {
    let mut out = Vec::with_capacity(props.len());
    for site in props {
        out.push(resolve_one(site, bindings, resolver, install, late));
    }
    let mut resolved_looks = Vec::with_capacity(looks.len());
    for decl in looks {
        let mut entries: Vec<(usize, LookEntry)> = Vec::new();
        for (pos, &i) in decl.props.iter().enumerate() {
            if let Some(Some((property, Some(value)))) =
                out.get(usize::try_from(i).unwrap_or(usize::MAX))
            {
                entries.push((
                    pos,
                    LookEntry {
                        property: *property,
                        value: value.clone(),
                    },
                ));
            }
        }
        // Sort by property ID, then line order; of equal IDs the later is kept.
        entries.sort_by_key(|(pos, e)| (e.property, *pos));
        let mut kept: Vec<LookEntry> = Vec::with_capacity(entries.len());
        let mut dups: Vec<(usize, u32)> = Vec::new();
        let mut it = entries.into_iter().peekable();
        while let Some((pos, e)) = it.next() {
            if it.peek().is_some_and(|(_, n)| n.property == e.property) {
                dups.push((pos, e.property));
            } else {
                kept.push(e);
            }
        }
        dups.sort_unstable();
        for (k, (pos, _)) in dups.iter().enumerate() {
            let site = decl
                .props
                .get(*pos)
                .and_then(|&i| props.get(usize::try_from(i).unwrap_or(usize::MAX)));
            if let Some(site) = site {
                late.push((
                    decl.line,
                    u32::MAX - 1,
                    k,
                    ResolveNote::Property {
                        line: decl.line,
                        property: written(&site.key),
                        note: PropertyNote::Duplicate,
                    },
                ));
            }
        }
        let mut look = Look { entries: kept };
        let mut cut = 0usize;
        while !look.entries.is_empty()
            && (look.encoded_len() > MAX_LOOK_BYTES || look.entries.len() > MAX_LOOK_ENTRIES)
        {
            look.entries.pop();
            cut += 1;
        }
        if cut > 0 {
            late.push((
                decl.line,
                u32::MAX,
                0,
                ResolveNote::Look {
                    line: decl.line,
                    cut,
                },
            ));
        }
        resolved_looks.push((!look.entries.is_empty()).then_some(look));
    }
    (out, resolved_looks)
}

/// The note of a dropped property value (§10.2 `property … note=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PropertyNote {
    /// The dictionary has no such property ID.
    Unknown,
    /// The value does not fit the property's kind (or the written kind differs).
    Mismatch,
    /// The property's type code is not supported.
    Unsupported,
    /// A later entry of the same look has the same property ID.
    Duplicate,
}

impl PropertyNote {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Mismatch => "mismatch",
            Self::Unsupported => "unsupported",
            Self::Duplicate => "duplicate",
        }
    }
}

fn resolve_one(
    site: &PropSite,
    bindings: &[Vec<ResolvedRecord>],
    resolver: &mut dyn SelectorResolver,
    install: bool,
    late: &mut Vec<LateNote>,
) -> ResolvedProp {
    let note = |late: &mut Vec<LateNote>, n: PropertyNote| {
        late.push((
            site.line,
            site.col,
            0,
            ResolveNote::Property {
                line: site.line,
                property: written(&site.key),
                note: n,
            },
        ));
    };
    let (id, kind) = match &site.key {
        PropKey::Label(l) => {
            if !install {
                return None;
            }
            let Some(info) = resolver.property(l) else {
                late.push((
                    site.line,
                    site.col,
                    0,
                    ResolveNote::Label {
                        line: site.line,
                        attribute: "property".to_owned(),
                        label: l.clone(),
                    },
                ));
                return None;
            };
            (info.id, info.kind)
        }
        PropKey::Pid(n, kind) => {
            if install {
                match resolver.property_by_id(*n) {
                    None => {
                        note(late, PropertyNote::Unknown);
                        return None;
                    }
                    Some(info) if info.id != *n || info.kind != *kind => {
                        note(late, PropertyNote::Mismatch);
                        return None;
                    }
                    Some(_) => {}
                }
            }
            (*n, *kind)
        }
    };
    if id == 0 {
        note(late, PropertyNote::Mismatch);
        return None;
    }
    if kind == PropertyKind::Unsupported {
        note(late, PropertyNote::Unsupported);
        return None;
    }
    let checked = match check(kind, &site.value, site.in_look) {
        Ok(c) => c,
        Err(_) => {
            note(late, PropertyNote::Mismatch);
            return None;
        }
    };
    let value = match checked {
        Checked::Value(v) => Some(v),
        Checked::Clear => None,
        Checked::Element(el) => {
            let rec = bindings
                .get(usize::try_from(el.sel).unwrap_or(usize::MAX))
                .and_then(|b| b.get(usize::try_from(el.index).unwrap_or(usize::MAX)))?;
            Some(PropertyValue::Did(rec.did))
        }
        Checked::Labels(kind, labels) => {
            if !install {
                return None;
            }
            let mut keys = Vec::with_capacity(labels.len());
            for l in &labels {
                match resolver.enum_value(LabelContext::PropertyKey(id), l) {
                    Some(k) => keys.push(k),
                    None => {
                        late.push((
                            site.line,
                            site.col,
                            0,
                            ResolveNote::Label {
                                line: site.line,
                                attribute: "value".to_owned(),
                                label: l.clone(),
                            },
                        ));
                        return None;
                    }
                }
            }
            match kind {
                PropertyKind::Enum => keys.first().map(|k| PropertyValue::Enum(*k)),
                PropertyKind::Bitfield32 | PropertyKind::Bitfield64 => {
                    let max = if kind == PropertyKind::Bitfield32 {
                        32
                    } else {
                        64
                    };
                    let mut mask = 0u64;
                    for k in keys {
                        if !(1..=max).contains(&k) {
                            note(late, PropertyNote::Mismatch);
                            return None;
                        }
                        mask |= 1u64 << (k - 1);
                    }
                    Some(if kind == PropertyKind::Bitfield32 {
                        PropertyValue::Bitfield32(u32::try_from(mask).unwrap_or(u32::MAX))
                    } else {
                        PropertyValue::Bitfield64(mask)
                    })
                }
                _ => None,
            }
        }
    };
    Some((id, value))
}

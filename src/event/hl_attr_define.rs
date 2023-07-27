use super::util::{maybe_field, parse_array, parse_bool, parse_map, parse_string, parse_u64};
use nvim_rs::Value;
use std::{fmt::Debug, vec::IntoIter};

#[derive(Debug, Clone)]
pub struct HlAttrDefine {
    pub hl_attrs: Vec<HlAttr>,
}

impl TryFrom<IntoIter<Value>> for HlAttrDefine {
    type Error = HlAttrDefineParseError;

    fn try_from(values: IntoIter<Value>) -> Result<Self, Self::Error> {
        let hl_attrs: Result<Vec<_>, _> = values.into_iter().map(HlAttr::try_from).collect();
        Ok(Self {
            hl_attrs: hl_attrs?,
        })
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum HlAttrDefineParseError {
    #[error("Error parsing HlAttr")]
    HlAttr,
    #[error("Error parsing Attributes")]
    Attributes,
    #[error("Error parsing Info")]
    Info,
    #[error("Error parsing Kind")]
    Kind,
}

/// Add a highlight with id to the highlight table
#[derive(Debug, Clone)]
pub struct HlAttr {
    pub id: u64,
    /// Highlights in RGB format
    pub rgb_attr: Attributes,
    /// Highlights in terminal 256-color codes
    pub cterm_attr: Attributes,
    /// A semantic description of the highlights active in a cell. Ordered by priority from low to
    /// high.
    pub info: Vec<Info>,
}

impl TryFrom<Value> for HlAttr {
    type Error = HlAttrDefineParseError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        use HlAttrDefineParseError::HlAttr as HlAttrError;
        let mut iter = parse_array(value).ok_or(HlAttrError)?.into_iter();
        let mut next = move || iter.next().ok_or(HlAttrError);
        Ok(Self {
            id: parse_u64(next()?).ok_or(HlAttrError)?,
            rgb_attr: Attributes::try_from(next()?)?,
            cterm_attr: Attributes::try_from(next()?)?,
            info: parse_array(next()?)
                .ok_or(HlAttrError)?
                .into_iter()
                .map(Info::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

#[derive(Clone, Copy, Default)]
pub struct Attributes {
    /// foreground color.
    pub foreground: Option<u64>,
    /// background color.
    pub background: Option<u64>,
    /// color to use for various underlines, when present.
    pub special: Option<u64>,
    /// reverse video. Foreground and background colors are switched.
    pub reverse: Option<bool>,
    /// italic text.
    pub italic: Option<bool>,
    /// bold text.
    pub bold: Option<bool>,
    /// struckthrough text.
    pub strikethrough: Option<bool>,
    /// underlined text. The line has special color.
    pub underline: Option<bool>,
    /// undercurled text. The curl has special color.
    pub undercurl: Option<bool>,
    /// double underlined text. The lines have special color.
    pub underdouble: Option<bool>,
    /// underdotted text. The dots have special color.
    pub underdotted: Option<bool>,
    /// underdashed text. The dashes have special color.
    pub underdashed: Option<bool>,
    /// alternative font.
    pub altfont: Option<bool>,
    /// Blend level (0-100). Could be used by UIs to support blending floating windows to the
    /// background or to signal a transparent cursor
    pub blend: Option<u64>,
}

impl TryFrom<Value> for Attributes {
    type Error = HlAttrDefineParseError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let inner = move || -> Option<Self> {
            let mut out = Self::default();
            for pair in parse_map(value)? {
                let (k, v) = pair;
                let k = parse_string(k)?;
                match k.as_str() {
                    "foreground" => out.foreground = Some(parse_u64(v)?),
                    "background" => out.background = Some(parse_u64(v)?),
                    "special" => out.special = Some(parse_u64(v)?),
                    "reverse" => out.reverse = Some(parse_bool(v)?),
                    "italic" => out.italic = Some(parse_bool(v)?),
                    "bold" => out.bold = Some(parse_bool(v)?),
                    "strikethrough" => out.strikethrough = Some(parse_bool(v)?),
                    "underline" => out.underline = Some(parse_bool(v)?),
                    "undercurl" => out.undercurl = Some(parse_bool(v)?),
                    "underdouble" => out.underdouble = Some(parse_bool(v)?),
                    "underdotted" => out.underdotted = Some(parse_bool(v)?),
                    "underdashed" => out.underdashed = Some(parse_bool(v)?),
                    "altfont" => out.altfont = Some(parse_bool(v)?),
                    "blend" => out.blend = Some(parse_u64(v)?),
                    _ => eprintln!("Unknown highlight attribute: {k}"),
                }
            }
            Some(out)
        };
        inner().ok_or(HlAttrDefineParseError::Attributes)
    }
}

impl Debug for Attributes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Attributes");
        maybe_field(&mut s, "foreground", self.foreground);
        maybe_field(&mut s, "background", self.background);
        maybe_field(&mut s, "special", self.special);
        maybe_field(&mut s, "reverse", self.reverse);
        maybe_field(&mut s, "italic", self.italic);
        maybe_field(&mut s, "bold", self.bold);
        maybe_field(&mut s, "strikethrough", self.strikethrough);
        maybe_field(&mut s, "underline", self.underline);
        maybe_field(&mut s, "undercurl", self.undercurl);
        maybe_field(&mut s, "underdouble", self.underdouble);
        maybe_field(&mut s, "underdotted", self.underdotted);
        maybe_field(&mut s, "underdashed", self.underdashed);
        maybe_field(&mut s, "altfont", self.altfont);
        maybe_field(&mut s, "blend", self.blend);
        s.finish()
    }
}

/// A semantic description of the highlights active in a cell. Activated by the ext_hlstate
/// extension.
#[derive(Debug, Clone)]
pub struct Info {
    pub kind: Kind,
    /// Highlight name from highlight-groups. Only for "ui" kind.
    pub ui_name: Option<String>,
    /// Name of the
    pub hi_name: Option<String>,
    /// Unique numeric id representing this item.
    pub id: Option<u64>,
}

impl TryFrom<Value> for Info {
    type Error = HlAttrDefineParseError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        use HlAttrDefineParseError::Info as InfoError;
        let mut kind = None;
        let mut ui_name = None;
        let mut hi_name = None;
        let mut id = None;
        let map = parse_map(value).ok_or(InfoError)?;
        for (k, v) in map {
            let k = parse_string(k).ok_or(InfoError)?;
            match k.as_str() {
                "kind" => kind = Some(Kind::try_from(v)?),
                "ui_name" => ui_name = Some(parse_string(v).ok_or(InfoError)?),
                "hi_name" => hi_name = Some(parse_string(v).ok_or(InfoError)?),
                "id" => id = Some(parse_u64(v).ok_or(InfoError)?),
                _ => eprintln!("Unrecognized hlstate keyword: {k}"),
            }
        }
        let kind = kind.ok_or(InfoError)?;
        Ok(Self {
            kind,
            ui_name,
            hi_name,
            id,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Kind {
    /// Builtin UI highlight.
    Ui,
    /// Highlight applied to a buffer by a syntax declaration or other runtime/plugin functionality
    /// such as nvim_buf_add_highlight()
    Syntax,
    /// highlight from a process running in a terminal-emulator. Contains no further semantic
    /// information.
    Terminal,
}

impl TryFrom<Value> for Kind {
    type Error = HlAttrDefineParseError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        let inner = move || -> Option<Self> {
            let s = parse_string(value)?;
            match s.as_str() {
                "ui" => Some(Self::Ui),
                "syntax" => Some(Self::Syntax),
                "terminal" => Some(Self::Terminal),
                _ => None,
            }
        };
        inner().ok_or(HlAttrDefineParseError::Kind)
    }
}

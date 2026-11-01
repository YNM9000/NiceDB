use krilla::text::Font;
use rustybuzz::ttf_parser::Face as Metrics;

use crate::PdfStyle;

const SANS: &[u8] = include_bytes!("../fonts/ibm-plex-sans-400.ttf");
const SANS_BOLD: &[u8] = include_bytes!("../fonts/ibm-plex-sans-600.ttf");
const COND: &[u8] = include_bytes!("../fonts/ibm-plex-sans-condensed-400.ttf");
const COND_BOLD: &[u8] = include_bytes!("../fonts/ibm-plex-sans-condensed-600.ttf");
const SERIF_BOLD: &[u8] = include_bytes!("../fonts/source-serif-4-600.ttf");
const MONO: &[u8] = include_bytes!("../fonts/ibm-plex-mono-400.ttf");

pub(crate) struct Face {
    pub font: Font,
    metrics: Metrics<'static>,
    upem: f32,
}

impl Face {
    fn load(data: &'static [u8]) -> Option<Self> {
        Self::load_at(data, 0)
    }

    fn load_at(data: &'static [u8], index: u32) -> Option<Self> {
        let metrics = Metrics::parse(data, index).ok()?;
        let upem = f32::from(metrics.units_per_em());
        Some(Self {
            font: Font::new(data.into(), index)?,
            metrics,
            upem,
        })
    }

    pub fn has(&self, c: char) -> bool {
        self.metrics.glyph_index(c).is_some()
    }

    pub fn width(&self, text: &str, size: f32) -> f32 {
        let units: f32 = text
            .chars()
            .map(|c| {
                self.metrics
                    .glyph_index(c)
                    .and_then(|g| self.metrics.glyph_hor_advance(g))
                    .map_or(self.upem / 2.0, f32::from)
            })
            .sum();
        units / self.upem * size
    }
}

pub(crate) struct Faces {
    pub body: Face,
    pub strong: Face,
    pub display: Face,
    pub mono: Face,
    pub fallback: Vec<Face>,
}

const SYSTEM_FALLBACKS: &[(&str, u32)] = &[
    ("/usr/share/fonts/google-noto/NotoSans-Regular.ttf", 0),
    ("/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf", 0),
    ("/usr/share/fonts/noto/NotoSans-Regular.ttf", 0),
    ("/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf", 0),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
    (
        "/usr/share/fonts/google-droid-sans-fonts/DroidSansFallbackFull.ttf",
        0,
    ),
    (
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        0,
    ),
    (
        "/usr/share/fonts/google-droid-sans-fonts/DroidKufi-Regular.ttf",
        0,
    ),
    (
        "/usr/share/fonts/google-droid-sans-fonts/DroidSansHebrew-Regular.ttf",
        0,
    ),
    (
        "/usr/share/fonts/google-droid-sans-fonts/DroidSansDevanagari-Regular.ttf",
        0,
    ),
    (
        "/usr/share/fonts/google-droid-sans-fonts/DroidSansThai.ttf",
        0,
    ),
    (
        "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf",
        0,
    ),
    (
        "/usr/share/fonts/truetype/noto/NotoSansHebrew-Regular.ttf",
        0,
    ),
    (
        "/usr/share/fonts/truetype/noto/NotoSansDevanagari-Regular.ttf",
        0,
    ),
    (
        "/usr/share/fonts/google-noto/NotoNaskhArabic-Regular.ttf",
        0,
    ),
    (
        "/usr/share/fonts/truetype/noto/NotoNaskhArabic-Regular.ttf",
        0,
    ),
    ("/usr/share/fonts/google-noto/NotoSansHebrew-Regular.ttf", 0),
    (
        "/usr/share/fonts/google-noto/NotoSansDevanagari-Regular.ttf",
        0,
    ),
    ("/usr/share/fonts/google-noto/NotoSansThai-Regular.ttf", 0),
    ("/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
    ("/Library/Fonts/Arial Unicode.ttf", 0),
    ("/System/Library/Fonts/Supplemental/Arial.ttf", 0),
    ("C:\\Windows\\Fonts\\segoeui.ttf", 0),
    ("C:\\Windows\\Fonts\\arial.ttf", 0),
    ("C:\\Windows\\Fonts\\msyh.ttc", 0),
    ("C:\\Windows\\Fonts\\msgothic.ttc", 0),
    ("C:\\Windows\\Fonts\\malgun.ttf", 0),
];

fn system_fonts() -> &'static [(&'static [u8], u32)] {
    static FONTS: std::sync::OnceLock<Vec<(&'static [u8], u32)>> = std::sync::OnceLock::new();
    FONTS.get_or_init(|| {
        SYSTEM_FALLBACKS
            .iter()
            .filter_map(|(path, index)| {
                let data = std::fs::read(path).ok()?;
                Metrics::parse(&data, *index).ok()?;
                Some((&*Box::leak(data.into_boxed_slice()), *index))
            })
            .collect()
    })
}

impl Faces {
    pub fn of(style: PdfStyle) -> Option<Self> {
        let (body, strong, display) = match style {
            PdfStyle::Registro => (SANS, SANS_BOLD, SERIF_BOLD),
            PdfStyle::Banco => (SANS, SANS_BOLD, COND_BOLD),
            PdfStyle::Tavola | PdfStyle::Listino => (COND, COND_BOLD, COND_BOLD),
        };
        Some(Self {
            body: Face::load(body)?,
            strong: Face::load(strong)?,
            display: Face::load(display)?,
            mono: Face::load(MONO)?,
            fallback: system_fonts()
                .iter()
                .filter_map(|(data, index)| Face::load_at(data, *index))
                .collect(),
        })
    }
}

pub(crate) fn runs<'a>(
    primary: &'a Face,
    fallback: &'a [Face],
    text: &str,
) -> Vec<(&'a Face, String)> {
    let mut out: Vec<(&'a Face, String)> = Vec::new();
    for c in text.chars() {
        let (face, piece) = if c == '\t' || c.is_control() || primary.has(c) {
            (primary, printable(primary, &c.to_string()))
        } else if let Some(f) = fallback.iter().find(|f| f.has(c)) {
            (f, c.to_string())
        } else {
            (primary, printable(primary, &c.to_string()))
        };
        match out.last_mut() {
            Some((last, s)) if std::ptr::eq(*last, face) => s.push_str(&piece),
            _ => out.push((face, piece)),
        }
    }
    out.retain(|(_, s)| !s.is_empty());
    out
}

pub(crate) fn printable(face: &Face, text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c == '\t' {
            out.push_str("    ");
        } else if c.is_control() {
        } else if face.has(c) {
            out.push(c);
        } else {
            match c {
                '\u{2018}' | '\u{2019}' | '\u{201A}' => out.push('\''),
                '\u{201C}' | '\u{201D}' | '\u{201E}' => out.push('"'),
                '\u{2013}' | '\u{2014}' | '\u{2212}' => out.push('-'),
                '\u{2026}' => out.push_str("..."),
                '\u{2192}' => out.push_str("->"),
                '\u{00A0}' | '\u{202F}' | '\u{2009}' => out.push(' '),
                _ => out.push('?'),
            }
        }
    }
    out
}

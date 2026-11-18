//! The fonts the app draws with.
//!
//! egui only ships a Latin font. Without more, a translated Arabic or Chinese
//! sentence would arrive as a row of empty boxes, so the app looks in the
//! Windows font folder for fonts that cover those scripts.
//!
//! The small fonts (Latin, Arabic) are read when the app starts. The big ones
//! (Chinese, Japanese, Korean) are only read when a character of that script
//! actually shows up, so the app does not carry 50 MB of font data around for
//! nothing.

use egui::{
    Context, FallbackRequest, FontData, FontDefinitions, FontFamily, FontInsert, FontPriority,
    InsertFontFamily,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// What the app found on this machine.
#[derive(Debug, Clone, Default)]
pub struct FontReport {
    /// The fonts that were installed.
    pub installed: Vec<String>,
    /// True when the main font has a weight axis, so the app can use real
    /// weights instead of only colour to build a hierarchy.
    pub variable: bool,
}

/// The weight axis of the main font, when it has one.
pub const WEIGHT_AXIS: &str = "wght";

/// Fonts that are read right away: they are small and cover most languages.
const EAGER_FONTS: &[(&str, &[&str])] = &[
    ("segoe-ui-variable", &["SegUIVar.ttf", "segoeui.ttf"]),
    ("segoe-ui", &["segoeui.ttf"]),
    ("consolas", &["consola.ttf"]),
];

/// Fonts that are only read when a character of their script shows up.
struct LazyFont {
    name: &'static str,
    files: &'static [&'static str],
    /// Which characters the font is offered for.
    matches: fn(char) -> bool,
}

const LAZY_FONTS: &[LazyFont] = &[
    LazyFont {
        name: "yahei",
        files: &["msyh.ttc", "simsun.ttc"],
        matches: is_han,
    },
    LazyFont {
        name: "jhenghei",
        files: &["msjh.ttc"],
        matches: is_han,
    },
    LazyFont {
        name: "gothic",
        files: &["msgothic.ttc", "meiryo.ttc"],
        matches: |ch| is_han(ch) || is_kana(ch),
    },
    LazyFont {
        name: "malgun",
        files: &["malgun.ttf"],
        matches: is_hangul,
    },
    LazyFont {
        name: "segoe-emoji",
        files: &["seguiemj.ttf"],
        matches: |ch| (ch as u32) >= 0x1F000,
    },
];

fn is_han(ch: char) -> bool {
    matches!(ch as u32,
        0x3000..=0x303F // punctuation used in Chinese and Japanese
        | 0x3400..=0x4DBF // extension A
        | 0x4E00..=0x9FFF // the ideographs themselves
        | 0xF900..=0xFAFF // compatibility ideographs
        | 0xFF00..=0xFFEF // full width forms
    )
}

fn is_kana(ch: char) -> bool {
    matches!(ch as u32, 0x3040..=0x30FF)
}

fn is_hangul(ch: char) -> bool {
    matches!(ch as u32,
        0x1100..=0x11FF // jamo
        | 0x3130..=0x318F // compatibility jamo
        | 0xAC00..=0xD7AF // syllables
    )
}

/// Where Windows keeps its fonts.
fn font_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(windir) = std::env::var("WINDIR") {
        dirs.push(PathBuf::from(windir).join("Fonts"));
    }
    dirs.push(PathBuf::from(r"C:\Windows\Fonts"));
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        dirs.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
    }
    dirs
}

/// Read the first of the given files that exists.
fn read_font(files: &[&str]) -> Option<FontData> {
    for directory in font_directories() {
        for file in files {
            let path = directory.join(file);
            if let Ok(bytes) = std::fs::read(&path) {
                if !bytes.is_empty() {
                    return Some(FontData::from_owned(bytes));
                }
            }
        }
    }
    None
}

/// Put the fonts in place and report what was found.
pub fn install(ctx: &Context) -> FontReport {
    let mut report = FontReport::default();
    let mut definitions = FontDefinitions::default();

    // The main font comes first, so Latin text looks like a Windows app and
    // not like a drawing program.
    let mut proportional: Vec<String> = Vec::new();
    let mut monospace: Vec<String> = Vec::new();

    for (name, files) in EAGER_FONTS {
        let Some(data) = read_font(files) else {
            continue;
        };
        let variable = data
            .variation_axes()
            .iter()
            .any(|axis| axis.tag == WEIGHT_AXIS);
        if *name == "segoe-ui-variable" {
            report.variable = variable;
        }
        definitions
            .font_data
            .insert((*name).to_string(), std::sync::Arc::new(data));
        report.installed.push((*name).to_string());
        if *name == "consolas" {
            monospace.push((*name).to_string());
        } else {
            proportional.push((*name).to_string());
        }
    }

    // Our fonts come first, the ones egui ships stay as a last resort.
    if let Some(chain) = definitions.families.get_mut(&FontFamily::Proportional) {
        let mut merged = proportional.clone();
        for name in chain.iter() {
            if !merged.contains(name) {
                merged.push(name.clone());
            }
        }
        *chain = merged;
    }
    if let Some(chain) = definitions.families.get_mut(&FontFamily::Monospace) {
        let mut merged = monospace.clone();
        for name in chain.iter() {
            if !merged.contains(name) {
                merged.push(name.clone());
            }
        }
        *chain = merged;
    }

    ctx.set_fonts(definitions);

    // The big fonts are read only when a character of their script shows up.
    let provider = std::sync::Arc::new(LazyFontProvider {
        cache: Mutex::new(HashMap::new()),
    });
    for font in LAZY_FONTS {
        ctx.add_font_provider(std::sync::Arc::new(ScriptFontProvider {
            font,
            cache: std::sync::Arc::clone(&provider),
        }));
    }

    report
}

/// Keeps the font data of the lazily loaded fonts.
struct LazyFontProvider {
    cache: Mutex<HashMap<&'static str, Option<FontData>>>,
}

/// Offers one font, but only for the characters it is meant for.
struct ScriptFontProvider {
    font: &'static LazyFont,
    cache: std::sync::Arc<LazyFontProvider>,
}

impl egui::FontProvider for ScriptFontProvider {
    fn font_for(&self, request: &FallbackRequest<'_>) -> Option<FontInsert> {
        let ch = request.cluster.chars().next()?;
        if !(self.font.matches)(ch) {
            return None;
        }
        let data = {
            let mut cache = self.cache.cache.lock().ok()?;
            cache
                .entry(self.font.name)
                .or_insert_with(|| read_font(self.font.files))
                .clone()?
        };
        Some(FontInsert::new(
            self.font.name,
            data,
            vec![InsertFontFamily {
                family: request.family.clone(),
                priority: FontPriority::Lowest,
            }],
        ))
    }
}

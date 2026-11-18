//! Reading the text under the pointer with the Windows accessibility APIs
//! (UI Automation).
//!
//! No OCR is used anywhere: only text that Windows exposes as accessible
//! text can be picked up. Controls that expose nothing simply return `None`
//! and the app stays quiet.

use uiautomation::core::{UIAutomation, UIElement};
use uiautomation::patterns::{UILegacyIAccessiblePattern, UITextPattern, UIValuePattern};
use uiautomation::types::{ControlType, Point, TextUnit};

/// Text found under the pointer plus the rectangle it came from.
#[derive(Debug, Clone)]
pub struct GrabbedText {
    pub text: String,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl GrabbedText {
    /// Is the given screen point inside the area the text came from?
    #[allow(dead_code)]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
}

/// Holds the UI Automation connection. It is created once and reused, because
/// creating it is comparatively slow.
pub struct TextGrabber {
    automation: Option<UIAutomation>,
}

impl TextGrabber {
    pub fn new() -> Self {
        Self {
            automation: UIAutomation::new().ok(),
        }
    }

    /// True when the accessibility APIs could be reached at all.
    pub fn is_available(&self) -> bool {
        self.automation.is_some()
    }

    /// Pick up the text that sits under the given screen point.
    pub fn grab(&self, x: i32, y: i32) -> Option<GrabbedText> {
        let automation = self.automation.as_ref()?;
        let walker = automation.create_tree_walker().ok();
        let point = Point::new(x, y);
        let element = automation.element_from_point(point).ok()?;

        // 1. Text patterns (the usual way text is exposed).
        let mut current = Some(element.clone());
        for _ in 0..6 {
            let element = match current {
                Some(ref e) => e,
                None => break,
            };
            if let Some(pattern) = element.get_pattern::<UITextPattern>().ok() {
                if let Some(text) = text_from_pattern(&pattern, point) {
                    if has_letters(&text) {
                        return Some(wrap(text, element));
                    }
                }
            }
            current = match walker.as_ref() {
                Some(walker) => walker.get_parent(element).ok(),
                None => None,
            };
        }

        // 2. The accessible name of the element itself.
        if let Some(text) = name_of(&element) {
            if has_letters(&text) {
                return Some(wrap(text, &element));
            }
        }

        None
    }
}

/// Text without any letter is not worth sending to a translator.
fn has_letters(text: &str) -> bool {
    text.chars().any(|c| c.is_alphabetic())
}

fn wrap(text: String, element: &UIElement) -> GrabbedText {
    let rect = element.get_bounding_rectangle().ok();
    GrabbedText {
        text,
        left: rect.map(|r| r.get_left()).unwrap_or(0),
        top: rect.map(|r| r.get_top()).unwrap_or(0),
        right: rect.map(|r| r.get_right()).unwrap_or(0),
        bottom: rect.map(|r| r.get_bottom()).unwrap_or(0),
    }
}

/// Prefer the paragraph around the pointer, then the line, then the word.
/// A single character is not worth translating, so short results fall through
/// to the text of the whole element.
fn text_from_pattern(pattern: &UITextPattern, point: Point) -> Option<String> {
    if let Ok(range) = pattern.get_range_from_point(point) {
        for (unit, max) in [
            (TextUnit::Paragraph, 1200),
            (TextUnit::Line, 600),
            (TextUnit::Word, 200),
        ] {
            if let Some(text) = expanded_text(&range, unit, max) {
                if text.chars().count() >= 3 {
                    return Some(text);
                }
            }
        }
    }

    // Fall back to the text the pattern covers (the content of a text box).
    // Long texts (a whole web page for instance) are not useful: the pointer
    // is on one small piece of text, not on the whole document.
    if let Ok(document) = pattern.get_document_range() {
        if let Ok(text) = document.get_text(1500) {
            let cleaned = clean(&text);
            if !cleaned.is_empty() && cleaned.chars().count() <= 300 {
                return Some(cleaned);
            }
        }
    }

    None
}

fn expanded_text(range: &uiautomation::patterns::UITextRange, unit: TextUnit, max: i32) -> Option<String> {
    let copy = range.clone();
    if copy.expand_to_enclosing_unit(unit).is_err() {
        return None;
    }
    let text = copy.get_text(max).ok()?;
    let cleaned = clean(&text);
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

/// The accessible name, but only for controls that really carry text.
fn name_of(element: &UIElement) -> Option<String> {
    let control_type = element.get_control_type().ok()?;
    let text_like = matches!(
        control_type,
        ControlType::Text
            | ControlType::Edit
            | ControlType::Document
            | ControlType::Hyperlink
            | ControlType::ListItem
            | ControlType::Group
            | ControlType::Custom
    );
    if !text_like {
        return None;
    }

    // Text boxes often keep their content in the value pattern.
    if let Ok(value) = element.get_pattern::<UIValuePattern>() {
        if let Ok(raw) = value.get_value() {
            let cleaned = clean(&raw);
            if !cleaned.is_empty() {
                return Some(cleaned.chars().take(400).collect());
            }
        }
    }

    let name = element.get_name().ok()?;
    let cleaned = clean(&name);
    if cleaned.is_empty() {
        return None;
    }
    // Old-style controls sometimes expose their text through this pattern.
    if let Ok(legacy) = element.get_pattern::<UILegacyIAccessiblePattern>() {
        if let Ok(value) = legacy.get_value() {
            let value = clean(&value);
            if !value.is_empty() && value.len() > cleaned.len() {
                return Some(value.chars().take(400).collect());
            }
        }
    }
    Some(cleaned)
}

/// Tidy the raw string: collapse whitespace and drop control characters.
fn clean(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_space = false;
    for ch in raw.chars() {
        match ch {
            '\n' | '\r' | '\t' | '\u{0b}' | '\u{0c}' => {
                if !last_space {
                    out.push(' ');
                    last_space = true;
                }
            }
            c if (c as u32) < 0x20 && c != '\u{00a0}' => {}
            c => {
                let is_space = c == ' ' || c == '\u{00a0}';
                if is_space && last_space {
                    continue;
                }
                last_space = is_space;
                out.push(if c == '\u{00a0}' { ' ' } else { c });
            }
        }
    }
    out.trim().to_string()
}

//! Unicode Bidirectional (UAX #9) paragraph direction, logical line wrapping,
//! and per-line visual run ordering for `egui 0.37`.
//!
//! Why this module is needed:
//! 1. `egui 0.37` (`epaint`) segments paragraphs by font face before shaping.
//!    Because `SegUIVar.ttf` covers spaces, digits, and Latin while `segoeui.ttf`
//!    covers Arabic and Hebrew, `epaint` splits an Arabic sentence into separate
//!    font runs at every space and places those runs from left to right. Without
//!    per-line run ordering, an Arabic sentence has its first word on the far
//!    left and its last word on the far right (reversed word order).
//! 2. Conversely, reordering an entire paragraph before line wrapping puts the
//!    end of a multi-line Arabic paragraph on the first line.
//!
//! Per the Unicode Bidirectional Algorithm (UAX #9, Section 3.4):
//! - Paragraphs are first wrapped into lines in **logical** order so Line 1
//!   always begins with the start of the sentence and the last line ends with
//!   the end of the sentence.
//! - Each individual wrapped line then has its bidirectional runs resolved and
//!   ordered visually (Rules W7, N1–N2, I1–I2, L1–L4), while HarfBuzz shapes
//!   the letters inside each Arabic/Hebrew word token.

use crate::languages::{is_cjk, is_rtl_char};

/// A logical token within a paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    /// Contiguous Arabic/Hebrew letters and their combining marks.
    RtlWord,
    /// Contiguous strong left-to-right letters (or a single CJK ideograph).
    LtrWord,
    /// Digits (ASCII or Arabic-Indic) including internal decimal points or trailing `%`.
    Number,
    /// Horizontal whitespace (`' '`, `'\t'`, `'\u{00A0}'`).
    Whitespace,
    /// Single punctuation or symbol character (`.`, `,`, `،`, `؟`, `!`, `(`, `)`, etc.).
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BidiClass {
    L,
    R,
    EN,
    WS,
    ON,
}

/// True for Arabic-Indic and Extended Arabic-Indic digits (`٠..٩`, `۰..۹`).
/// In Unicode Bidi (UAX #9), these are numbers (`AN`/`EN`) rendered left-to-right,
/// not strong RTL letters.
fn is_arabic_indic_digit(ch: char) -> bool {
    matches!(ch, '\u{0660}'..='\u{0669}' | '\u{06F0}'..='\u{06F9}')
}

/// True for any ASCII or Arabic-Indic digit.
fn is_digit_char(ch: char) -> bool {
    ch.is_ascii_digit() || is_arabic_indic_digit(ch)
}

/// Punctuation marks in the Arabic Unicode block that should be treated as
/// neutral punctuation (`ON`/`CS`) rather than part of an Arabic word stem,
/// so UAX #9 places them on the proper trailing (left) side of the word or number.
fn is_rtl_block_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '،' // U+060C Arabic comma
        | '؛' // U+061B Arabic semicolon
        | '؟' // U+061F Arabic question mark
        | '٪' // U+066A Arabic percent sign
        | '؉' | '؊' | '؍' | '؞' | '۔'
    )
}

/// True for Unicode combining marks (harakat / tashkeel / accents) that attach
/// to the preceding base character.
fn is_combining_mark(ch: char) -> bool {
    matches!(
        ch as u32,
        0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x0591..=0x05BD
            | 0x05BF
            | 0x05C1..=0x05C2
            | 0x05C4..=0x05C5
            | 0x05C7
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06ED
            | 0x0711
            | 0x0730..=0x074A
            | 0x08D3..=0x08FF
            | 0xFE20..=0xFE2F
    )
}

/// True when `ch` is a strong right-to-left letter or mark.
pub fn is_strong_rtl_char(ch: char) -> bool {
    if matches!(ch, '\u{200F}' | '\u{202B}' | '\u{202E}' | '\u{2067}') {
        return true;
    }
    is_rtl_char(ch) && !is_arabic_indic_digit(ch) && !is_rtl_block_punctuation(ch)
}

/// True when `ch` is a strong left-to-right letter or mark.
fn is_strong_ltr_char(ch: char) -> bool {
    if matches!(ch, '\u{200E}' | '\u{202A}' | '\u{202D}' | '\u{2066}') {
        return true;
    }
    ch.is_alphabetic() && !is_rtl_char(ch)
}

/// True when `text` contains any right-to-left script character.
pub fn has_rtl(text: &str) -> bool {
    text.chars().any(|ch| is_rtl_char(ch) || is_strong_rtl_char(ch))
}

/// The base direction of a paragraph: the first strong letter decides it, just as
/// `unicode-bidi: plaintext` (UAX #9 Rules P2–P3) does in a browser. Digits and
/// punctuation are skipped, and text with no strong letter defaults to left-to-right.
pub fn base_is_rtl(text: &str) -> bool {
    for ch in text.chars() {
        if is_strong_rtl_char(ch) {
            return true;
        }
        if is_strong_ltr_char(ch) {
            return false;
        }
    }
    false
}

/// Tokenize a single paragraph (without `'\n'`) into logical bidirectional tokens.
pub fn tokenize_paragraph(paragraph: &str) -> Vec<Token> {
    let chars: Vec<char> = paragraph.chars().collect();
    let len = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // 1. Whitespace
        if ch.is_whitespace() {
            let start = i;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(Token {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::Whitespace,
            });
            continue;
        }

        // 2. Numbers (e.g. "123", "0.8", "50%", "1,000")
        if is_digit_char(ch) {
            let start = i;
            i += 1;
            while i < len {
                let c = chars[i];
                if is_digit_char(c) {
                    i += 1;
                } else if matches!(c, '.' | ',' | '\u{066B}' | '\u{066C}')
                    && i + 1 < len
                    && is_digit_char(chars[i + 1])
                {
                    // Internal decimal or thousands separator between two digits ("0.8")
                    i += 1;
                } else if matches!(c, '%' | '٪') {
                    // Trailing percent sign attached to a number ("50%")
                    i += 1;
                    break;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::Number,
            });
            continue;
        }

        // 3. Strong RTL word (Arabic / Hebrew letters + combining marks / tatweel / joiners)
        if is_strong_rtl_char(ch) {
            let start = i;
            i += 1;
            while i < len {
                let c = chars[i];
                if is_strong_rtl_char(c)
                    || is_combining_mark(c)
                    || matches!(c, '\u{0640}' | '\u{200C}' | '\u{200D}')
                {
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::RtlWord,
            });
            continue;
        }

        // 4. Strong LTR word (Latin, Cyrillic, Greek, or single CJK ideograph)
        if is_strong_ltr_char(ch) {
            if is_cjk(ch) {
                let start = i;
                i += 1;
                while i < len && is_combining_mark(chars[i]) {
                    i += 1;
                }
                tokens.push(Token {
                    text: chars[start..i].iter().collect(),
                    kind: TokenKind::LtrWord,
                });
                continue;
            }

            let start = i;
            i += 1;
            while i < len {
                let c = chars[i];
                if (is_strong_ltr_char(c) && !is_cjk(c)) || is_combining_mark(c) {
                    i += 1;
                } else if matches!(c, '.' | '-' | '_' | '/' | '\'' | '’')
                    && i + 1 < len
                    && ((is_strong_ltr_char(chars[i + 1]) && !is_cjk(chars[i + 1]))
                        || chars[i + 1].is_ascii_digit())
                {
                    // Keep domains, hyphenated words, contractions together ("Example.com")
                    i += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token {
                text: chars[start..i].iter().collect(),
                kind: TokenKind::LtrWord,
            });
            continue;
        }

        // 5. Neutral / punctuation character (plus any combining marks on it)
        let start = i;
        i += 1;
        while i < len && is_combining_mark(chars[i]) {
            i += 1;
        }
        tokens.push(Token {
            text: chars[start..i].iter().collect(),
            kind: TokenKind::Neutral,
        });
    }

    tokens
}

/// Mirror symmetric bracket and quotation characters when they sit at an odd (RTL)
/// embedding level (UAX #9 Rule L4).
fn mirror_char(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        other => other,
    }
}

/// Reorder the logical tokens of a **single wrapped line** into visual left-to-right
/// order according to UAX #9 (Rules W7, N1–N2, I1–I2, L1–L4).
///
/// The text inside each word token is never reversed character-by-character;
/// HarfBuzz shapes the characters inside each word.
pub fn reorder_line(tokens: &[Token], base_rtl: bool) -> Vec<String> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let n = tokens.len();
    let mut classes: Vec<BidiClass> = tokens
        .iter()
        .map(|t| match t.kind {
            TokenKind::RtlWord => BidiClass::R,
            TokenKind::LtrWord => BidiClass::L,
            TokenKind::Number => BidiClass::EN,
            TokenKind::Whitespace => BidiClass::WS,
            TokenKind::Neutral => BidiClass::ON,
        })
        .collect();

    // UAX #9 Rule W7: Search backward from each EN to the first strong type (R, L, or sor).
    // If L is found, change EN to L so phrases like "Hello 123" stay a single L run.
    let sor_eos = if base_rtl { BidiClass::R } else { BidiClass::L };
    let mut last_strong = sor_eos;
    for class in &mut classes {
        match *class {
            BidiClass::L | BidiClass::R => last_strong = *class,
            BidiClass::EN if last_strong == BidiClass::L => *class = BidiClass::L,
            _ => {}
        }
    }

    // UAX #9 Rules N1 & N2: Sequences of neutrals take the direction of the surrounding
    // text if both sides have the same direction (with EN acting as R for N1); otherwise
    // they take the paragraph embedding direction (sor_eos).
    let n1_dir = |c: BidiClass| -> Option<BidiClass> {
        match c {
            BidiClass::L => Some(BidiClass::L),
            BidiClass::R | BidiClass::EN => Some(BidiClass::R),
            BidiClass::WS | BidiClass::ON => None,
        }
    };

    let mut idx = 0;
    while idx < n {
        if matches!(classes[idx], BidiClass::WS | BidiClass::ON) {
            let start = idx;
            while idx < n && matches!(classes[idx], BidiClass::WS | BidiClass::ON) {
                idx += 1;
            }
            let end = idx;
            let prev_dir = (0..start)
                .rev()
                .find_map(|k| n1_dir(classes[k]))
                .unwrap_or(sor_eos);
            let next_dir = (end..n)
                .find_map(|k| n1_dir(classes[k]))
                .unwrap_or(sor_eos);
            let resolved = if prev_dir == next_dir {
                prev_dir
            } else {
                sor_eos
            };
            for item in &mut classes[start..end] {
                *item = resolved;
            }
        } else {
            idx += 1;
        }
    }

    // UAX #9 Rules I1 & I2: Resolve implicit embedding levels.
    let mut levels: Vec<u8> = classes
        .iter()
        .map(|c| {
            if !base_rtl {
                // Rule I1 (even base level 0)
                match c {
                    BidiClass::L => 0,
                    BidiClass::R => 1,
                    BidiClass::EN => 2,
                    BidiClass::WS | BidiClass::ON => 0,
                }
            } else {
                // Rule I2 (odd base level 1)
                match c {
                    BidiClass::R => 1,
                    BidiClass::L | BidiClass::EN => 2,
                    BidiClass::WS | BidiClass::ON => 1,
                }
            }
        })
        .collect();

    // UAX #9 Rule L1: Trailing whitespace on the line resets to the paragraph base level.
    let base_level = if base_rtl { 1u8 } else { 0u8 };
    for k in (0..n).rev() {
        if tokens[k].kind == TokenKind::Whitespace {
            levels[k] = base_level;
        } else {
            break;
        }
    }

    // UAX #9 Rule L4: Mirror symmetric brackets/quotes at odd resolved levels.
    let mut visual_tokens: Vec<String> = tokens
        .iter()
        .enumerate()
        .map(|(k, tok)| {
            if levels[k] % 2 == 1 && tok.kind == TokenKind::Neutral {
                tok.text.chars().map(mirror_char).collect()
            } else {
                tok.text.clone()
            }
        })
        .collect();

    // UAX #9 Rule L2: Reverse contiguous subsequences from the highest level down to 1.
    let max_level = levels.iter().copied().max().unwrap_or(0);
    for level in (1..=max_level).rev() {
        let mut start = 0;
        while start < n {
            if levels[start] >= level {
                let mut end = start + 1;
                while end < n && levels[end] >= level {
                    end += 1;
                }
                visual_tokens[start..end].reverse();
                levels[start..end].reverse();
                start = end;
            } else {
                start += 1;
            }
        }
    }

    visual_tokens
}

/// Group logical tokens into unbreakable word units (for line wrapping) and
/// whitespace tokens, wrap them in **logical order** so each line fits within
/// `max_width`, and then reorder each wrapped line visually with [`reorder_line`].
///
/// Because line wrapping happens in logical order *before* per-line visual reordering:
/// - Line 0 always starts with the beginning of the sentence.
/// - Subsequent lines continue top-to-bottom in natural reading order.
/// - Within each line, RTL words flow from right to left while embedded English
///   and numbers remain left-to-right.
pub fn wrap_and_reorder(
    paragraph: &str,
     base_rtl: bool,
    max_width: f32,
    mut measure: impl FnMut(&str) -> f32,
) -> Vec<Vec<String>> {
    let tokens = tokenize_paragraph(paragraph);
    if tokens.is_empty() {
        return vec![Vec::new()];
    }

    if !max_width.is_finite() || max_width <= 0.0 {
        return vec![reorder_line(&tokens, base_rtl)];
    }

    // Group non-whitespace tokens that have no spaces between them into unbreakable
    // word units (so trailing punctuation like "." or "،" or "!" never wraps onto a
    // new line by itself), while allowing breaks between CJK ideographs.
    enum Unit {
        Space(Token),
        Word(Vec<Token>),
    }

    let mut units: Vec<Unit> = Vec::new();
    let mut current_word: Vec<Token> = Vec::new();

    for token in tokens {
        if token.kind == TokenKind::Whitespace {
            if !current_word.is_empty() {
                units.push(Unit::Word(std::mem::take(&mut current_word)));
            }
            units.push(Unit::Space(token));
        } else {
            let is_cjk_token = token.kind == TokenKind::LtrWord
                && token.text.chars().next().is_some_and(is_cjk);
            if is_cjk_token && !current_word.is_empty() {
                // Allow a line break before a new CJK ideograph unless the previous
                // token was an opening bracket/quote.
                let prev_is_no_break = current_word.len() == 1
                    && current_word[0].kind == TokenKind::Neutral
                    && matches!(current_word[0].text.as_str(), "(" | "[" | "{" | "«" | "“" | "\"");
                if !prev_is_no_break {
                    units.push(Unit::Word(std::mem::take(&mut current_word)));
                }
            }
            current_word.push(token);
        }
    }
    if !current_word.is_empty() {
        units.push(Unit::Word(current_word));
    }

    let mut logical_lines: Vec<Vec<Token>> = Vec::new();
    let mut current_line: Vec<Token> = Vec::new();
    let mut current_width = 0.0f32;
    let mut pending_space: Option<(Token, f32)> = None;

    for unit in units {
        match unit {
            Unit::Space(space_token) => {
                if !current_line.is_empty() {
                    let w = measure(&space_token.text);
                    pending_space = Some((space_token, w));
                }
            }
            Unit::Word(word_tokens) => {
                let word_width: f32 = word_tokens.iter().map(|t| measure(&t.text)).sum();
                let space_width = pending_space.as_ref().map_or(0.0, |(_, w)| *w);

                if current_line.is_empty() {
                    current_line.extend(word_tokens);
                    current_width = word_width;
                } else if current_width + space_width + word_width <= max_width {
                    if let Some((space_tok, _)) = pending_space.take() {
                        current_line.push(space_tok);
                    }
                    current_line.extend(word_tokens);
                    current_width += space_width + word_width;
                } else {
                    pending_space = None;
                    logical_lines.push(std::mem::take(&mut current_line));
                    current_line.extend(word_tokens);
                    current_width = word_width;
                }
            }
        }
    }

    if !current_line.is_empty() || logical_lines.is_empty() {
        logical_lines.push(current_line);
    }

    logical_lines
        .into_iter()
        .map(|line_tokens| reorder_line(&line_tokens, base_rtl))
        .collect()
}

/// Convenience helper returning the visually ordered string for a single line.
#[allow(dead_code)]
pub fn visual_line(text: &str) -> String {
    let rtl = base_is_rtl(text);
    let tokens = tokenize_paragraph(text);
    reorder_line(&tokens, rtl).concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_and_hebrew_paragraphs_are_right_to_left() {
        assert!(base_is_rtl("ضع المؤشر على أي نص لقراءته بلغتك"));
        assert!(base_is_rtl("שלום, מה שלומך היום?"));
        // Digits and punctuation are neutral: the first strong letter wins.
        assert!(base_is_rtl("123 مرحبا"));
        assert!(base_is_rtl("! مرحبا"));
    }

    #[test]
    fn english_and_other_scripts_stay_left_to_right() {
        assert!(!base_is_rtl(
            "Rest the pointer on any text to read it in your language"
        ));
        assert!(!base_is_rtl("您好，您今天过得怎么样？"));
        // The first strong letter decides: English first means an LTR
        // paragraph even when Arabic appears later in the same line.
        assert!(!base_is_rtl("Hello مرحبا"));
    }

    #[test]
    fn text_without_a_strong_letter_defaults_to_left_to_right() {
        assert!(!base_is_rtl("123 !? ..."));
        assert!(!base_is_rtl(""));
    }

    #[test]
    fn direction_comes_from_the_first_strong_character() {
        assert!(base_is_rtl("50% مرحبا Example.com"));
        assert!(!base_is_rtl("50% Example.com مرحبا"));
    }

    #[test]
    fn arabic_sentence_reorders_words_from_right_to_left() {
        // Logical order: "ضع" first, "بلغتك" last.
        // Visual left-to-right order puts "بلغتك" on the far left and "ضع" on the far right,
        // so reading right-to-left yields "ضع المؤشر على أي نص لقراءته بلغتك".
        let text = "ضع المؤشر على أي نص لقراءته بلغتك";
        let tokens = tokenize_paragraph(text);
        let visual = reorder_line(&tokens, true);
        assert_eq!(visual.first().map(String::as_str), Some("بلغتك"));
        assert_eq!(visual.last().map(String::as_str), Some("ضع"));
        assert_eq!(
            visual.concat(),
            "بلغتك لقراءته نص أي على المؤشر ضع"
        );
    }

    #[test]
    fn mixed_arabic_english_numbers_and_punctuation_preserve_order() {
        // In an RTL sentence:
        // - Arabic words flow right-to-left
        // - Embedded "Hello 123" stays "Hello 123" (LTR)
        // - Trailing period sits at the far left end of the RTL sentence
        let text = "مرحبا Hello 123 النص.";
        let tokens = tokenize_paragraph(text);
        let visual = reorder_line(&tokens, true);
        assert_eq!(visual.concat(), ".النص Hello 123 مرحبا");
    }

    #[test]
    fn multiline_arabic_wraps_in_logical_order_top_to_bottom() {
        let paragraph = "ضع المؤشر على أي نص لقراءته بلغتك، وانتظر 0.8 ثانية. تظهر الترجمة إلى English في مربع صغير.";
        // Measure 1 unit per character, wrap at 35 chars per line:
        let lines = wrap_and_reorder(paragraph, true, 35.0, |s| s.chars().count() as f32);
        assert!(lines.len() >= 2, "expected multiple wrapped lines");

        // Line 0 (top line) must contain the beginning of the sentence ("ضع"),
        // placed at the rightmost visual token of Line 0!
        let first_line = &lines[0];
        assert_eq!(first_line.last().map(String::as_str), Some("ضع"));

        // The last line must contain the end of the sentence ("صغير" and "."),
        // with "." at the leftmost visual position of the last line!
        let last_line = lines.last().unwrap();
        assert_eq!(last_line.first().map(String::as_str), Some("."));
        assert!(last_line.iter().any(|t| t == "صغير"));
    }

    #[test]
    fn brackets_are_mirrored_in_rtl_context() {
        let text = "خدمة (بدون مفتاح)";
        let tokens = tokenize_paragraph(text);
        let visual = reorder_line(&tokens, true);
        assert_eq!(visual.concat(), "(مفتاح بدون) خدمة");
    }
}

//! Lyrics obfuscation for Suno: Unicode transformations to preserve lyric structure
//! and prevent automated content filter blocks.
//! Based on suno-lyrics-obfuscator (ProstoAndriuha, MIT).

use crate::i18n::t;

/// Obfuscation mode for lyrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ObfuscationMode {
    #[default]
    None,
    ZeroWidth,
    WordJoiner,
    SoftHyphen,
    Combined,
    Homoglyph,
}

impl ObfuscationMode {
    pub const ALL: [Self; 6] = [
        Self::None,
        Self::ZeroWidth,
        Self::WordJoiner,
        Self::SoftHyphen,
        Self::Combined,
        Self::Homoglyph,
    ];

    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::ZeroWidth,
            2 => Self::WordJoiner,
            3 => Self::SoftHyphen,
            4 => Self::Combined,
            5 => Self::Homoglyph,
            _ => Self::None,
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            Self::None => 0,
            Self::ZeroWidth => 1,
            Self::WordJoiner => 2,
            Self::SoftHyphen => 3,
            Self::Combined => 4,
            Self::Homoglyph => 5,
        }
    }
}

pub fn choices() -> Vec<String> {
    vec![
        t("None (original lyrics)"),
        t("Zero-width characters"),
        t("Word joiner"),
        t("Soft hyphens"),
        t("Combined (zero-width and soft hyphens)"),
        t("Cyrillic homoglyphs"),
    ]
}

const ZW_CHARS: [char; 3] = ['\u{200B}', '\u{200C}', '\u{200D}'];
const SOFT_HYPHEN: char = '\u{00AD}';
const WORD_JOINER: char = '\u{2060}';

/// Strips zero-width characters, soft hyphens and word joiners.
pub fn clean_special_characters(text: &str) -> String {
    text.chars()
        .filter(|&c| !matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{00AD}' | '\u{2060}'))
        .collect()
}

/// Checks if a line is a structural tag like [Verse 1], [Chorus], (Hook), etc.
pub fn is_structural_tag(line: &str) -> bool {
    let trimmed = line.trim();
    (trimmed.starts_with('[') && trimmed.ends_with(']') && trimmed.len() >= 2)
        || (trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() >= 2)
}

fn is_combining_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '\'' || is_combining_mark(c)
}

fn split_graphemes(word: &str) -> Vec<String> {
    let mut clusters = Vec::new();
    let mut current = String::new();
    for c in word.chars() {
        if is_combining_mark(c) && !current.is_empty() {
            current.push(c);
        } else {
            if !current.is_empty() {
                clusters.push(current);
                current = String::new();
            }
            current.push(c);
        }
    }
    if !current.is_empty() {
        clusters.push(current);
    }
    clusters
}

fn homoglyph_for(c: char) -> char {
    match c {
        'a' => 'а',
        'c' => 'с',
        'e' => 'е',
        'i' => 'і',
        'j' => 'ј',
        'o' => 'о',
        'p' => 'р',
        's' => 'ѕ',
        'x' => 'х',
        'y' => 'у',
        'A' => 'А',
        'B' => 'В',
        'C' => 'С',
        'E' => 'Е',
        'H' => 'Н',
        'I' => 'І',
        'J' => 'Ј',
        'K' => 'К',
        'M' => 'М',
        'O' => 'О',
        'P' => 'Р',
        'T' => 'Т',
        'X' => 'Х',
        'Y' => 'У',
        other => other,
    }
}

fn obfuscate_word(word: &str, mode: ObfuscationMode) -> String {
    let letters = split_graphemes(word);
    if letters.len() <= 1 {
        return word.to_string();
    }

    match mode {
        ObfuscationMode::None => word.to_string(),
        ObfuscationMode::ZeroWidth => {
            let mut out = String::new();
            for (index, char_str) in letters.iter().enumerate() {
                out.push_str(char_str);
                if index < letters.len() - 1 {
                    out.push(ZW_CHARS[index % ZW_CHARS.len()]);
                }
            }
            out
        }
        ObfuscationMode::WordJoiner => {
            let mut out = String::new();
            for (index, char_str) in letters.iter().enumerate() {
                out.push_str(char_str);
                if index < letters.len() - 1 {
                    out.push(WORD_JOINER);
                }
            }
            out
        }
        ObfuscationMode::SoftHyphen => {
            let mut out = String::new();
            for (index, char_str) in letters.iter().enumerate() {
                out.push_str(char_str);
                if index < letters.len() - 1 && index % 2 == 0 {
                    out.push(SOFT_HYPHEN);
                }
            }
            out
        }
        ObfuscationMode::Combined => {
            let mut out = String::new();
            for (index, char_str) in letters.iter().enumerate() {
                out.push_str(char_str);
                if index < letters.len() - 1 {
                    out.push(ZW_CHARS[index % ZW_CHARS.len()]);
                    out.push(SOFT_HYPHEN);
                }
            }
            out
        }
        ObfuscationMode::Homoglyph => {
            let mut out = String::new();
            for char_str in &letters {
                for c in char_str.chars() {
                    out.push(homoglyph_for(c));
                }
            }
            out
        }
    }
}

/// Transforms lyrics according to the chosen obfuscation mode.
/// If `preserve_tags` is true, lines like `[Verse 1]` or `(Chorus)` remain untouched.
pub fn obfuscate_lyrics(text: &str, mode: ObfuscationMode, preserve_tags: bool) -> String {
    if mode == ObfuscationMode::None || text.trim().is_empty() {
        return text.to_string();
    }
    let cleaned = clean_special_characters(text);
    let mut out_lines = Vec::new();

    for line in cleaned.lines() {
        if preserve_tags && is_structural_tag(line) {
            out_lines.push(line.to_string());
            continue;
        }

        let mut transformed_line = String::new();
        let mut word = String::new();

        for c in line.chars() {
            if is_word_char(c) {
                word.push(c);
            } else {
                if !word.is_empty() {
                    transformed_line.push_str(&obfuscate_word(&word, mode));
                    word.clear();
                }
                transformed_line.push(c);
            }
        }
        if !word.is_empty() {
            transformed_line.push_str(&obfuscate_word(&word, mode));
        }

        out_lines.push(transformed_line);
    }

    out_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_mode_returns_unmodified() {
        let input = "Hello world\n[Chorus]\nSing it loud!";
        assert_eq!(obfuscate_lyrics(input, ObfuscationMode::None, true), input);
    }

    #[test]
    fn preserves_structural_tags() {
        let input = "[Verse 1]\nHello world\n(Chorus)\nDon't stop";
        let out = obfuscate_lyrics(input, ObfuscationMode::ZeroWidth, true);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "[Verse 1]");
        assert!(lines[1].contains('\u{200B}'));
        assert_eq!(lines[2], "(Chorus)");
        assert!(lines[3].contains('\u{200B}'));
    }

    #[test]
    fn zero_width_steganography() {
        let input = "Sing";
        let out = obfuscate_lyrics(input, ObfuscationMode::ZeroWidth, false);
        assert_eq!(out, "S\u{200B}i\u{200C}n\u{200D}g");
    }

    #[test]
    fn word_joiner() {
        let input = "Sing";
        let out = obfuscate_lyrics(input, ObfuscationMode::WordJoiner, false);
        assert_eq!(out, "S\u{2060}i\u{2060}n\u{2060}g");
    }

    #[test]
    fn soft_hyphen() {
        let input = "Sing";
        let out = obfuscate_lyrics(input, ObfuscationMode::SoftHyphen, false);
        assert_eq!(out, "S\u{00AD}in\u{00AD}g");
    }

    #[test]
    fn combined_mode() {
        let input = "Sing";
        let out = obfuscate_lyrics(input, ObfuscationMode::Combined, false);
        assert_eq!(out, "S\u{200B}\u{00AD}i\u{200C}\u{00AD}n\u{200D}\u{00AD}g");
    }

    #[test]
    fn homoglyphs_cyrillic_swap() {
        let input = "Cats and dogs";
        let out = obfuscate_lyrics(input, ObfuscationMode::Homoglyph, false);
        assert!(out.contains('С')); // Cyrillic Es
        assert!(out.contains('а')); // Cyrillic a
        assert!(out.contains('о')); // Cyrillic o
        assert!(!out.contains('C')); // Latin C was swapped
    }

    #[test]
    fn cleans_prior_special_chars() {
        let dirty = "S\u{200B}i\u{2060}n\u{00AD}g";
        assert_eq!(clean_special_characters(dirty), "Sing");
    }
}

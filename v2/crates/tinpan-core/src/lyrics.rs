//! Suno's word timings, grouped into sung lines, and the .lrc file made from them, as in Tin Pan 1.

use crate::clip::SunoClip;

/// One sung word and when it is sung, in seconds (GET /api/gen/{id}/aligned_lyrics/v2/).
#[derive(Debug, Clone, PartialEq)]
pub struct AlignedWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// One sung line and when it starts, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub start: f64,
    pub text: String,
}

const WORDS_PER_LINE_WITHOUT_BREAKS: usize = 8;

/// Groups the words into lines: at Suno's line breaks, or every 8 words when it has none.
pub fn lines_from(words: &[AlignedWord]) -> Vec<LyricLine> {
    let has_breaks = words.iter().any(|w| w.word.contains('\n'));
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut start: Option<f64> = None;
    let mut count = 0;
    let flush = |line: &mut String, start: &mut Option<f64>, count: &mut usize, lines: &mut Vec<LyricLine>| {
        let text = line.trim();
        if let (false, Some(at)) = (text.is_empty(), *start) {
            lines.push(LyricLine { start: at, text: text.to_string() });
        }
        line.clear();
        *start = None;
        *count = 0;
    };
    for word in words {
        let text = word.word.replace('\r', "");
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                flush(&mut line, &mut start, &mut count, &mut lines);
            }
            if part.trim().is_empty() {
                continue;
            }
            start.get_or_insert(word.start);
            line.push_str(part);
            if !part.ends_with(' ') {
                line.push(' ');
            }
            count += 1;
        }
        if !has_breaks && count >= WORDS_PER_LINE_WITHOUT_BREAKS {
            flush(&mut line, &mut start, &mut count, &mut lines);
        }
    }
    flush(&mut line, &mut start, &mut count, &mut lines);
    lines
}

/// mm:ss.xx, as LRC players expect.
pub fn stamp(seconds: f64) -> String {
    let hundredths = (seconds.max(0.0) * 100.0).floor() as u64;
    format!("{:02}:{:02}.{:02}", hundredths / 6000, hundredths / 100 % 60, hundredths % 100)
}

/// An .lrc file: one timed line per sung line.
pub fn lrc(clip: &SunoClip, words: &[AlignedWord]) -> String {
    let mut out = format!("[ti:{}]\n[ar:Suno]\n", clip.display_title());
    if let Some(length) = clip.duration().filter(|d| *d > 0.0) {
        out += &format!("[length:{}]\n", stamp(length));
    }
    for line in lines_from(words) {
        out += &format!("[{}]{}\n", stamp(line.start), line.text);
    }
    out
}

/// The line without section tags such as [Verse 1], for speaking; empty when it was only a tag.
pub fn spoken(text: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in text.chars() {
        match c {
            '[' => in_tag = true,
            ']' if in_tag => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Which line is being sung at a time in the song: says each one once, and only the newest after a jump.
pub struct LyricsCursor {
    lines: Vec<LyricLine>,
    index: Option<usize>,
}

impl LyricsCursor {
    /// Speech takes a moment to start, so lines are said this much early.
    pub const LEAD: f64 = 0.15;

    pub fn new(lines: Vec<LyricLine>) -> Self {
        Self { lines, index: None }
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The line to say now, or None when the line hasn't changed (or has nothing to say).
    pub fn advance(&mut self, position: f64) -> Option<String> {
        let index = self.lines.iter().rposition(|l| l.start <= position + Self::LEAD);
        if index == self.index {
            return None;
        }
        self.index = index;
        let spoken = spoken(&self.lines[index?].text);
        (!spoken.is_empty()).then_some(spoken)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_says_each_line_once_and_skips_tags() {
        let lines = vec![
            LyricLine { start: 1.0, text: "[Verse 1]".into() },
            LyricLine { start: 2.0, text: "Rain on the [soft] roof".into() },
            LyricLine { start: 5.0, text: "Sun again".into() },
        ];
        let mut cursor = LyricsCursor::new(lines);
        assert_eq!(cursor.advance(0.5), None);
        assert_eq!(cursor.advance(1.0), None);
        assert_eq!(cursor.advance(1.9).as_deref(), Some("Rain on the roof"));
        assert_eq!(cursor.advance(3.0), None);
        // A jump says only the newest line.
        assert_eq!(cursor.advance(9.0).as_deref(), Some("Sun again"));
        assert_eq!(cursor.advance(2.5).as_deref(), Some("Rain on the roof"));
    }

    fn word(word: &str, start: f64) -> AlignedWord {
        AlignedWord { word: word.into(), start, end: start + 0.4 }
    }

    #[test]
    fn lines_break_where_suno_breaks_them() {
        let words = [word("Hello ", 1.0), word("world\n", 1.5), word("again", 3.25)];
        assert_eq!(lines_from(&words), vec![LyricLine { start: 1.0, text: "Hello world".into() }, LyricLine { start: 3.25, text: "again".into() }]);
    }

    #[test]
    fn without_breaks_every_8_words_is_a_line() {
        let words: Vec<_> = (0..10).map(|i| word("la", f64::from(i))).collect();
        let lines = lines_from(&words);
        assert_eq!((lines.len(), lines[1].start), (2, 8.0));
    }

    #[test]
    fn lrc_has_the_title_length_and_timed_lines() {
        let clip = SunoClip::from_value(&serde_json::json!({"id": "c", "title": "Song", "metadata": {"duration": 125.5}})).unwrap();
        assert_eq!(lrc(&clip, &[word("Hi\n", 61.237)]), "[ti:Song]\n[ar:Suno]\n[length:02:05.50]\n[01:01.23]Hi\n");
    }
}

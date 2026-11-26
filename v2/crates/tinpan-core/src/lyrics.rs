//! Suno's word timings, grouped into sung lines, and the .lrc file made from them, as in Tin Pan 1.

use crate::clip::SunoClip;

/// One sung word and when it is sung, in seconds (GET /api/gen/{id}/aligned_lyrics/v2/).
#[derive(Debug, Clone, PartialEq)]
pub struct AlignedWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// One sung line, when it starts and when its last word ends, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricLine {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

const WORDS_PER_LINE_WITHOUT_BREAKS: usize = 8;

/// Groups the words into lines: at Suno's line breaks, or every 8 words when it has none.
pub fn lines_from(words: &[AlignedWord]) -> Vec<LyricLine> {
    let has_breaks = words.iter().any(|w| w.word.contains('\n'));
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut start: Option<f64> = None;
    let mut end = 0.0;
    let mut count = 0;
    let flush = |line: &mut String, start: &mut Option<f64>, end: f64, count: &mut usize, lines: &mut Vec<LyricLine>| {
        let text = line.trim();
        if let (false, Some(at)) = (text.is_empty(), *start) {
            lines.push(LyricLine { start: at, end: end.max(at), text: text.to_string() });
        }
        line.clear();
        *start = None;
        *count = 0;
    };
    for word in words {
        let text = word.word.replace('\r', "");
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                flush(&mut line, &mut start, end, &mut count, &mut lines);
            }
            if part.trim().is_empty() {
                continue;
            }
            start.get_or_insert(word.start);
            end = word.end;
            line.push_str(part);
            if !part.ends_with(' ') {
                line.push(' ');
            }
            count += 1;
        }
        if !has_breaks && count >= WORDS_PER_LINE_WITHOUT_BREAKS {
            flush(&mut line, &mut start, end, &mut count, &mut lines);
        }
    }
    flush(&mut line, &mut start, end, &mut count, &mut lines);
    lines
}

/// How far ahead in Suno's timed lines a written line is looked for.
const MATCH_AHEAD: usize = 4;
/// Seconds given to each untimed line when there's nothing to place it against.
const SECONDS_PER_UNTIMED_LINE: f64 = 3.0;
/// Lines shorter than this, once reduced to letters and digits, only match when they're the same.
const SHORTEST_PARTIAL_MATCH: usize = 8;

/// A line reduced to lower-case letters and digits, single spaces between words, without section tags.
fn plain(text: &str) -> String {
    spoken(text)
        .chars()
        .map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn same_line(timed: &str, written: &str) -> bool {
    if timed.is_empty() || written.is_empty() {
        return false;
    }
    let (short, long) = if timed.len() <= written.len() { (timed, written) } else { (written, timed) };
    short == long || (short.len() >= SHORTEST_PARTIAL_MATCH && long.contains(short))
}

/// Suno's timed lines plus the lines of the written lyrics Suno didn't time (hums, "ooh"s, backing vocals),
/// each given a time between the timed lines around it. Written lines are matched to timed ones in order;
/// when none match, the timed lines come back as they were. `length` is the song's, in seconds, if known.
pub fn with_untimed_lines(timed: Vec<LyricLine>, written: &str, length: Option<f64>) -> Vec<LyricLine> {
    let written: Vec<&str> = written.lines().map(str::trim).filter(|l| !spoken(l).is_empty()).collect();
    let timed_plain: Vec<String> = timed.iter().map(|l| plain(&l.text)).collect();
    // For each written line, the timed line it matches, if any. Suno sometimes sings two written lines as one
    // timed line, so a written line inside the last matched timed line counts as matched too.
    let mut next: usize = 0;
    let matches: Vec<Option<usize>> = written
        .iter()
        .map(|w| {
            let w = plain(w);
            if let Some(last) = next.checked_sub(1)
                && w.len() >= SHORTEST_PARTIAL_MATCH
                && timed_plain[last].contains(&w)
            {
                return Some(last);
            }
            let found = (next..timed.len().min(next + MATCH_AHEAD)).find(|&k| same_line(&timed_plain[k], &w));
            if let Some(k) = found {
                next = k + 1;
            }
            found
        })
        .collect();
    if matches.iter().all(Option::is_none) {
        return timed;
    }

    let mut added = Vec::new();
    let mut i = 0;
    while i < written.len() {
        if matches[i].is_some() {
            i += 1;
            continue;
        }
        let first = i;
        while i < written.len() && matches[i].is_none() {
            i += 1;
        }
        let group = &written[first..i];
        let n = group.len() as f64;
        // The timed line after the group, and the one sung just before it.
        let after = matches[i..].iter().find_map(|m| *m);
        let before = match after {
            Some(0) => None,
            Some(m) => Some(m - 1),
            None => timed.len().checked_sub(1),
        };
        let times: Vec<f64> = match (before, after) {
            (None, Some(m)) => (0..group.len()).map(|j| (timed[m].start - SECONDS_PER_UNTIMED_LINE * (n - j as f64)).max(0.0)).collect(),
            (Some(b), _) => {
                let ended = timed[b].end;
                let to = match after {
                    Some(m) => timed[m].start,
                    None => length.filter(|l| *l > ended).unwrap_or(ended + SECONDS_PER_UNTIMED_LINE * n),
                };
                // Suno's lines can overlap; then the untimed ones go between the two lines' starts.
                let from = if to > ended { ended } else { timed[b].start };
                let to = if to > from { to } else { from + SECONDS_PER_UNTIMED_LINE * n };
                (0..group.len()).map(|j| from + (to - from) * j as f64 / n).collect()
            }
            (None, None) => Vec::new(),
        };
        added.extend(group.iter().zip(times).map(|(text, start)| LyricLine { start, end: start, text: (*text).to_string() }));
    }
    let mut lines = timed;
    lines.extend(added);
    lines.sort_by(|a, b| a.start.total_cmp(&b.start));
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
            line(1.0, 1.0, "[Verse 1]"),
            line(2.0, 4.0, "Rain on the [soft] roof"),
            line(5.0, 6.0, "Sun again"),
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
        assert_eq!(texts_and_starts(&lines_from(&words)), vec![("Hello world".into(), 1.0), ("again".into(), 3.25)]);
    }

    #[test]
    fn without_breaks_every_8_words_is_a_line() {
        let words: Vec<_> = (0..10).map(|i| word("la", f64::from(i))).collect();
        let lines = lines_from(&words);
        assert_eq!((lines.len(), lines[1].start), (2, 8.0));
    }

    fn line(start: f64, end: f64, text: &str) -> LyricLine {
        LyricLine { start, end, text: text.into() }
    }

    fn texts_and_starts(lines: &[LyricLine]) -> Vec<(String, f64)> {
        lines.iter().map(|l| (l.text.clone(), (l.start * 100.0).round() / 100.0)).collect()
    }

    #[test]
    fn untimed_hums_go_between_the_lines_around_them() {
        let timed = vec![line(10.0, 13.0, "Rain on the roof,"), line(19.0, 21.0, "Light in the hall")];
        let written = "[Verse 1]\nRain on the roof\nHmm hmm hmm hmm\n(ooh, oh)\n\nLight in the hall\n";
        let lines = with_untimed_lines(timed, written, None);
        assert_eq!(
            texts_and_starts(&lines),
            vec![("Rain on the roof,".into(), 10.0), ("Hmm hmm hmm hmm".into(), 13.0), ("(ooh, oh)".into(), 16.0), ("Light in the hall".into(), 19.0)]
        );
    }

    #[test]
    fn hums_before_the_first_timed_line_come_just_before_it() {
        let timed = vec![line(12.0, 14.0, "Rain on the roof")];
        let lines = with_untimed_lines(timed, "Hmm hmm hmm\nHeya eeee\nRain on the roof", None);
        assert_eq!(texts_and_starts(&lines), vec![("Hmm hmm hmm".into(), 6.0), ("Heya eeee".into(), 9.0), ("Rain on the roof".into(), 12.0)]);
    }

    #[test]
    fn hums_after_the_last_timed_line_fill_the_rest_of_the_song() {
        let timed = vec![line(10.0, 12.0, "Rain on the roof")];
        let lines = with_untimed_lines(timed.clone(), "Rain on the roof\nHmm hmm\nOoh", Some(18.0));
        assert_eq!(texts_and_starts(&lines), vec![("Rain on the roof".into(), 10.0), ("Hmm hmm".into(), 12.0), ("Ooh".into(), 15.0)]);
        // Without the song's length, each takes a few seconds.
        let lines = with_untimed_lines(timed, "Rain on the roof\nHmm hmm\nOoh", None);
        assert_eq!(texts_and_starts(&lines)[2], ("Ooh".into(), 15.0));
    }

    #[test]
    fn lines_suno_timed_are_not_said_twice() {
        let timed = vec![line(10.0, 12.0, "Hmm hmm hmm hmm"), line(13.0, 15.0, "Rain on the roof")];
        let lines = with_untimed_lines(timed.clone(), "Hmm hmm hmm hmm\nRain on the roof", None);
        assert_eq!(lines, timed);
    }

    #[test]
    fn two_written_lines_suno_sang_as_one_are_not_said_again() {
        let timed = vec![line(10.0, 14.0, "Rain on the roof light in the hall"), line(20.0, 22.0, "Sun again")];
        let lines = with_untimed_lines(timed, "Rain on the roof\nLight in the hall\nHmm hmm hmm\nSun again", None);
        assert_eq!(
            texts_and_starts(&lines),
            vec![("Rain on the roof light in the hall".into(), 10.0), ("Hmm hmm hmm".into(), 14.0), ("Sun again".into(), 20.0)]
        );
    }

    #[test]
    fn written_lyrics_that_match_nothing_timed_are_left_out() {
        let timed = vec![line(10.0, 12.0, "Rain on the roof")];
        assert_eq!(with_untimed_lines(timed.clone(), "Something else entirely\nHmm", None), timed);
        assert_eq!(with_untimed_lines(timed.clone(), "", None), timed);
        assert!(with_untimed_lines(Vec::new(), "Hmm hmm\nRain", None).is_empty());
    }

    #[test]
    fn lines_end_where_their_last_word_ends() {
        let words = [AlignedWord { word: "Hello ".into(), start: 1.0, end: 1.25 }, AlignedWord { word: "world\n".into(), start: 1.5, end: 2.5 }, word("again", 3.0)];
        assert_eq!(lines_from(&words)[0].end, 2.5);
    }

    #[test]
    fn lrc_has_the_title_length_and_timed_lines() {
        let clip = SunoClip::from_value(&serde_json::json!({"id": "c", "title": "Song", "metadata": {"duration": 125.5}})).unwrap();
        assert_eq!(lrc(&clip, &[word("Hi\n", 61.237)]), "[ti:Song]\n[ar:Suno]\n[length:02:05.50]\n[01:01.23]Hi\n");
    }
}

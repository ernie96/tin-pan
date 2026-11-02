//! Tin Pan's translations, read from the same translations.json as Tin Pan 1 (each English text with
//! its Romanian, Ukrainian and Spanish), so nothing has to be translated twice. Text is written in
//! English in the code; anything without a translation stays in English. Format strings keep the
//! 1.x style, "{0}" and "{1}", so the existing translations fit as they are.

use std::collections::HashMap;
use std::sync::OnceLock;
#[cfg(not(test))]
use std::sync::RwLock;

const TRANSLATIONS: &str = include_str!("../../../../TinPan/Localization/translations.json");
const TRANSLATED: [&str; 3] = ["ro", "uk", "es"];

/// Code, the language's own name, and its English name.
pub const LANGUAGES: [(&str, &str, &str); 5] = [
    ("", "Automatic (system language)", "Automatic (system language)"),
    ("en", "English", "English"),
    ("ro", "Română", "Romanian"),
    ("uk", "Українська", "Ukrainian"),
    ("es", "Español", "Spanish"),
];

fn table() -> &'static HashMap<String, Vec<String>> {
    static TABLE: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    TABLE.get_or_init(|| serde_json::from_str(TRANSLATIONS).unwrap_or_default())
}

// The language is the whole app's. In tests it's each test's own, since tests run side by side.
#[cfg(not(test))]
fn column() -> Option<usize> {
    *chosen().read().unwrap()
}

#[cfg(not(test))]
fn set_column(column: Option<usize>) {
    *chosen().write().unwrap() = column;
}

#[cfg(not(test))]
fn chosen() -> &'static RwLock<Option<usize>> {
    static COLUMN: OnceLock<RwLock<Option<usize>>> = OnceLock::new();
    COLUMN.get_or_init(|| RwLock::new(None))
}

#[cfg(test)]
thread_local! {
    static COLUMN: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn column() -> Option<usize> {
    COLUMN.get()
}

#[cfg(test)]
fn set_column(column: Option<usize>) {
    COLUMN.set(column);
}

/// Chooses the language: "ro", "uk", "es", "en", or "" for the system's (`system` is its code, such as "ro").
pub fn use_language(code: &str, system: &str) {
    let chosen = if code.trim().is_empty() { system } else { code }.trim().to_lowercase();
    let chosen = chosen.split(['-', '_']).next().unwrap_or("").to_string();
    set_column(TRANSLATED.iter().position(|c| *c == chosen));
}

/// The language in use: "en", "ro", "uk" or "es".
pub fn current() -> &'static str {
    column().map_or("en", |i| TRANSLATED[i])
}

/// The translation of an English text.
pub fn t(english: &str) -> String {
    lookup(english).unwrap_or_else(|| english.to_string())
}

fn lookup(key: &str) -> Option<String> {
    let column = column()?;
    table().get(key).and_then(|row| row.get(column)).filter(|s| !s.is_empty()).cloned()
}

/// Translates a format string ("Saved {0}.") and fills in its {0}, {1}, ...
pub fn tf(english_format: &str, args: &[&dyn std::fmt::Display]) -> String {
    fill(&t(english_format), args)
}

/// Fills "{0}" style places (also "{0:0}" and the like; the format part is ignored).
pub fn fill(format: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(format.len() + 16);
    let mut rest = format;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        if let Some(stripped) = after.strip_prefix('{') {
            out.push('{');
            rest = stripped;
            continue;
        }
        match after.find('}') {
            Some(close) => {
                let inner = &after[..close];
                let index = inner.split([':', ',']).next().and_then(|n| n.trim().parse::<usize>().ok());
                // "{0:N0}" groups the digits, as Tin Pan 1's .NET formatting does; other formats are ignored.
                let grouped = inner.ends_with(":N0");
                match index.and_then(|i| args.get(i)) {
                    Some(arg) => {
                        let text = arg.to_string();
                        match text.parse::<i64>() {
                            Ok(number) if grouped => out.push_str(&group_digits(number)),
                            _ => out.push_str(&text),
                        }
                    }
                    None => {
                        out.push('{');
                        out.push_str(inner);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(&rest.replace("}}", "}"));
    out
}

/// A whole number with the language's thousands separator: 1,234 (English and Spanish), 1.234
/// (Romanian), 1 234 (Ukrainian).
pub fn group_digits(value: i64) -> String {
    let separator = match current() {
        "ro" => ".",
        "uk" => "\u{a0}",
        _ => ",",
    };
    let digits = value.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    if value < 0 { format!("-{out}") } else { out }
}

/// A date as each language writes it in full: "November 9, 2026", "9 noiembrie 2026", "9 листопада 2026 р.",
/// "9 de noviembre de 2026".
pub fn long_date(date: chrono::NaiveDate) -> String {
    use chrono::Datelike;
    const EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    const RO: [&str; 12] = ["ianuarie", "februarie", "martie", "aprilie", "mai", "iunie", "iulie", "august", "septembrie", "octombrie", "noiembrie", "decembrie"];
    // The month as it reads after a day number (genitive).
    const UK: [&str; 12] = ["січня", "лютого", "березня", "квітня", "травня", "червня", "липня", "серпня", "вересня", "жовтня", "листопада", "грудня"];
    const ES: [&str; 12] = ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"];
    let (day, month, year) = (date.day(), date.month0() as usize, date.year());
    match current() {
        "ro" => format!("{day} {} {year}", RO[month]),
        "uk" => format!("{day} {} {year} р.", UK[month]),
        "es" => format!("{day} de {} de {year}", ES[month]),
        _ => format!("{} {day}, {year}", EN[month]),
    }
}

/// A count with the right noun form: `plural(n, "{0} song", "{0} songs")`. Translations hold their
/// forms separated by "|" (Romanian: one|few|other, Ukrainian: one|few|many, Spanish: one|other).
pub fn plural(count: i64, english_one: &str, english_other: &str) -> String {
    let form = match lookup(&format!("{english_one}|{english_other}")) {
        Some(row) => {
            let forms: Vec<&str> = row.split('|').collect();
            forms[plural_index(count).min(forms.len() - 1)].to_string()
        }
        None if count == 1 => english_one.to_string(),
        None => english_other.to_string(),
    };
    fill(&form, &[&count])
}

/// Which plural form a count takes in the current language.
pub fn plural_index(count: i64) -> usize {
    let n = count.unsigned_abs();
    match current() {
        "ro" => {
            if n == 1 {
                0
            } else if n == 0 || (1..=19).contains(&(n % 100)) {
                1
            } else {
                2
            }
        }
        "uk" => {
            if n % 10 == 1 && n % 100 != 11 {
                0
            } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
                1
            } else {
                2
            }
        }
        _ => usize::from(n != 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_numbered_places_and_keeps_unknown_ones() {
        assert_eq!(fill("Saved {0} of {1}.", &[&"a", &3]), "Saved a of 3.");
        assert_eq!(fill("Volume {0:0}.", &[&80]), "Volume 80.");
        assert_eq!(fill("{2} stays", &[&1]), "{2} stays");
    }

    // One test, since the language is shared by the whole process and tests run at the same time.
    #[test]
    fn plurals_in_each_language_and_the_translation_table_loads() {
        use_language("en", "");
        assert_eq!(plural(1, "{0} song.", "{0} songs."), "1 song.");
        assert_eq!(plural(3, "{0} song.", "{0} songs."), "3 songs.");
        assert!(table().len() > 100, "translations.json should load");
        assert_eq!((group_digits(1250), group_digits(999), group_digits(-1234567)), ("1,250".into(), "999".into(), "-1,234,567".into()));
        assert_eq!(plural(1250, "{0:N0} credit left.", "{0:N0} credits left."), "1,250 credits left.");
        let date = chrono::NaiveDate::from_ymd_opt(2026, 11, 9).unwrap();
        assert_eq!(long_date(date), "November 9, 2026");
        use_language("", "ro-RO");
        assert_eq!(current(), "ro");
        assert_eq!((group_digits(1250), long_date(date)), ("1.250".into(), "9 noiembrie 2026".into()));
        assert_ne!(t("Stopped."), "Stopped.");
        assert_eq!((plural_index(1), plural_index(5), plural_index(20)), (0, 1, 2));
        use_language("uk", "");
        assert_eq!((plural_index(21), plural_index(22), plural_index(25)), (0, 1, 2));
        use_language("en", "");
    }
}

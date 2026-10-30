// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The data behind Intl.DisplayNames (plan decision 5 of Phase 5): single
//! name lookups and the locale display patterns. JS performs every
//! operation of the draft, including the composition of language names.
//!
//! - Language, dialect, region, script and variant names come from
//!   icu_experimental's display name data, read as data structs, and
//!   currency names from its `CurrencyDisplaynameV1`. ICU4X's data leaves
//!   out every name that equals its code (en's short `US`, `Thai`);
//!   `display-names.txt` lists those as `eq` entries, which apply only
//!   where ICU4X has no name.
//! - Calendar names, date-time field names and the locale display pattern
//!   come from `display-names.txt` alone: ICU4X has none of them.
//!
//! The entry of display-names.txt for a locale is the first one ICU4X's
//! locale fallback reaches from its language identifier, then its parents
//! in the file. The file is ours, so a malformed line is a panic.

use crate::cldr::{body_lines, fallback_key, locale_parent};
use crate::locale::language_id;
use crate::number::load;
use crate::supported::{CALENDARS, CALENDAR_ALIASES};
use icu::locale::subtags::{Language, Region, Script, Variant};
use icu::locale::LanguageIdentifier;
use icu_experimental::dimension::provider::currency::displayname::CurrencyDisplaynameV1;
use icu_experimental::displaynames::provider::*;
use icu_experimental::provider::Baked as ExperimentalBaked;
use icu_provider::prelude::*;
use potential_utf::PotentialUtf8;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::sync::OnceLock;
use zerovec::ZeroMap;

/// The ECMA-402 date-time field codes, the codes of `f.<code>.<style>` keys.
const FIELDS: [&str; 12] = [
    "era",
    "year",
    "quarter",
    "month",
    "weekOfYear",
    "weekday",
    "day",
    "dayPeriod",
    "hour",
    "minute",
    "second",
    "timeZoneName",
];

/// One locale of display-names.txt.
struct NamesLocale {
    /// The locale whose entries this one overrides; None for und.
    parent: Option<&'static str>,
    /// Key to value, where they differ from the parent chain's.
    entries: HashMap<&'static str, &'static str>,
}

/// The parsed display-names.txt, by locale tag.
type NamesTable = HashMap<&'static str, NamesLocale>;

/// Parse display-names.txt.
fn parse_names(text: &'static str) -> NamesTable {
    const FILE: &str = "display-names.txt";
    let mut table = NamesTable::new();
    let mut current: Option<&'static str> = None;
    for line in body_lines(text) {
        if let Some(rest) = line.strip_prefix("locale ") {
            let f: Vec<&'static str> = rest.split(' ').collect();
            let [tag, parent] = f[..] else {
                panic!("{FILE}: bad line {line:?}")
            };
            let parent = locale_parent(FILE, tag, parent, |p| table.contains_key(p));
            assert_eq!(parent.is_none(), tag == "und", "{FILE}: {line:?}");
            let fresh = NamesLocale {
                parent,
                entries: HashMap::new(),
            };
            assert!(
                table.insert(tag, fresh).is_none(),
                "{FILE}: repeated locale {tag}"
            );
            current = Some(tag);
        } else {
            let tag = current.unwrap_or_else(|| panic!("{FILE}: bad line {line:?}"));
            // Not trimmed: a value may begin or end with a space.
            let (key, value) = line
                .split_once('\t')
                .unwrap_or_else(|| panic!("{FILE}: bad line {line:?}"));
            assert!(
                key.starts_with("eq.") == value.is_empty(),
                "{FILE}: bad line {line:?}"
            );
            assert!(
                table
                    .get_mut(tag)
                    .unwrap()
                    .entries
                    .insert(key, value)
                    .is_none(),
                "{FILE}: repeated {line:?}"
            );
        }
    }
    let und = table.get("und").unwrap_or_else(|| panic!("{FILE}: no und"));
    assert!(
        und.entries.contains_key("lp") && und.entries.contains_key("ls"),
        "{FILE}: und has no patterns"
    );
    table
}

/// The parsed display-names.txt.
fn names_table() -> &'static NamesTable {
    static T: OnceLock<NamesTable> = OnceLock::new();
    T.get_or_init(|| parse_names(include_str!("../data/display-names.txt")))
}

/// The entries of display-names.txt that apply to one locale.
struct Entries {
    /// The parsed file.
    table: &'static NamesTable,
    /// The locale's entry: the first one ICU4X's fallback reaches.
    tag: String,
}

impl Entries {
    /// The entries of `table` for the language identifier `id`.
    fn new(table: &'static NamesTable, id: &LanguageIdentifier) -> Entries {
        let tag = fallback_key(id, |t| table.contains_key(t));
        Entries { table, tag }
    }

    /// The value of `key` in the nearest locale of the chain that has it.
    fn get(&self, key: &str) -> Option<&'static str> {
        let mut t = Some(self.tag.as_str());
        while let Some(current) = t {
            let locale = &self.table[current];
            if let Some(value) = locale.entries.get(key) {
                return Some(value);
            }
            t = locale.parent;
        }
        None
    }

    /// Whether the chain lists the name of `code` (CLDR's key) of `kind` in
    /// `style` ("long" or "short") as equal to the code.
    fn is_code_equal(&self, kind: &str, style: &str, code: &str) -> bool {
        self.get(&format!("eq.{kind}.{style}.{code}")).is_some()
    }
}

/// The entries of a table for a language identifier, made on first use:
/// most lookups find their name in ICU4X and never need them.
struct LazyEntries<'a> {
    /// The parsed file: names_table(), or a test's own.
    table: &'static NamesTable,
    /// The language identifier.
    id: &'a LanguageIdentifier,
    /// The entries, once made.
    entries: OnceCell<Entries>,
}

impl<'a> LazyEntries<'a> {
    /// The entries of `table` for `id`, not yet made.
    fn new(table: &'static NamesTable, id: &'a LanguageIdentifier) -> LazyEntries<'a> {
        LazyEntries {
            table,
            id,
            entries: OnceCell::new(),
        }
    }

    /// The entries, made on the first call.
    fn entries(&self) -> &Entries {
        self.entries
            .get_or_init(|| Entries::new(self.table, self.id))
    }
}

/// A name from ICU4X's `names` and `short_names` maps (None where ICU4X
/// has no data for the locale, or no short names for the kind), or the
/// code-equal list of display-names.txt: for long, `names`, then the listed
/// long entry; for short and narrow (`short`), `short_names`, the listed
/// short entry, `names`, then the listed long entry. `key` is the code as
/// ICU4X's maps key it. `code` is the code as display-names.txt keys it,
/// which is also the name a listed entry stands for, and `kind` is the kind
/// of its `eq` keys, which `entries` holds.
fn from_maps<'d, K>(
    names: Option<&ZeroMap<'d, K, str>>,
    short_names: Option<&ZeroMap<'d, K, str>>,
    key: &K,
    entries: &LazyEntries,
    kind: &str,
    code: &str,
    short: bool,
) -> Option<String>
where
    K: zerovec::maps::ZeroMapKV<'d> + Ord + ?Sized,
{
    let found = |map: Option<&ZeroMap<'d, K, str>>| map?.get(key).map(str::to_owned);
    // Read the file only on a miss: ICU4X has most names.
    let listed = |style| {
        entries
            .entries()
            .is_code_equal(kind, style, code)
            .then(|| code.to_owned())
    };
    if short {
        if let Some(name) = found(short_names).or_else(|| listed("short")) {
            return Some(name);
        }
    }
    found(names).or_else(|| listed("long"))
}

/// The display name of `code` for the language identifier of `locale` (a
/// canonical tag), by `kind`:
///
/// - `"language"`: a language subtag (`"en"`, `"und"`);
/// - `"dialect"`: a `language-script` or `language-region` key, CLDR's
///   dialect names (`"en-US"` is `"American English"`);
/// - `"region"`, `"script"`: a region or script subtag;
/// - `"variant"`: a variant subtag, lowercase;
/// - `"currency"`: an uppercase ISO 4217 code;
/// - `"calendar"`: one of the 16 supported calendars, or the alias
///   `ethiopic-amete-alem` or `islamicc` (its canonical calendar's name);
/// - `"dateTimeField"`: one of ECMA-402's 12 date-time field codes.
///
/// `style` is `"long"`, `"short"` or `"narrow"`; currency and calendar
/// names have one style. Language, dialect, region and script names read
/// CLDR's `alt="short"` name for short and narrow, then the default one.
/// Some(None) when there is no name, including for a code that is not of
/// its kind (`"ZZZZ"` as a region); None for any other kind or style, or
/// if `locale` does not parse.
pub fn name(locale: &str, kind: &str, code: &str, style: &str) -> Option<Option<String>> {
    let short = match style {
        "long" => false,
        "short" | "narrow" => true,
        _ => return None,
    };
    let id = language_id(locale)?;
    let data_locale = DataLocale::from(&id);
    let entries = LazyEntries::new(names_table(), &id);
    // Load `M`, whose data has `names` and `short_names` maps.
    macro_rules! names_and_short {
        ($marker:ty, $key:expr, $code:expr) => {{
            let data = load::<$marker>(&ExperimentalBaked, &data_locale, "");
            let data = data.as_ref().map(|p| p.get());
            from_maps(
                data.map(|d| &d.names),
                data.map(|d| &d.short_names),
                $key,
                &entries,
                kind,
                $code,
                short,
            )
        }};
    }
    Some(match kind {
        "language" => Language::try_from_str(code).ok().and_then(|language| {
            names_and_short!(
                LocaleNamesLanguageV0,
                &language.to_tinystr().to_unvalidated(),
                language.as_str()
            )
        }),
        "dialect" => dialect_key(code).and_then(|key| {
            names_and_short!(LocaleNamesLocaleV0, PotentialUtf8::from_str(&key), &key)
        }),
        "region" => Region::try_from_str(code).ok().and_then(|region| {
            names_and_short!(
                LocaleNamesRegionV0,
                &region.to_tinystr().to_unvalidated(),
                region.as_str()
            )
        }),
        "script" => Script::try_from_str(code).ok().and_then(|script| {
            names_and_short!(
                LocaleNamesScriptV0,
                &script.to_tinystr().to_unvalidated(),
                script.as_str()
            )
        }),
        "variant" => Variant::try_from_str(code).ok().and_then(|variant| {
            let data = load::<LocaleNamesVariantV0>(&ExperimentalBaked, &data_locale, "");
            variant_name(data.as_ref().map(|p| p.get()), variant, &entries)
        }),
        "currency" => (code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase()))
            .then(|| load::<CurrencyDisplaynameV1>(&ExperimentalBaked, &data_locale, code))
            .flatten()
            .map(|p| p.get().to_string()),
        "calendar" => {
            let canonical = CALENDAR_ALIASES
                .iter()
                .find(|(alias, _)| *alias == code)
                .map_or(code, |(_, canonical)| canonical);
            CALENDARS
                .contains(&canonical)
                .then(|| entries.entries().get(&format!("ca.{canonical}")))
                .flatten()
                .map(str::to_owned)
        }
        "dateTimeField" => FIELDS
            .contains(&code)
            .then(|| entries.entries().get(&format!("f.{code}.{style}")))
            .flatten()
            .map(str::to_owned),
        _ => return None,
    })
}

/// The name of `variant` from ICU4X's variant names `data` (None where
/// ICU4X has none for the locale), or the code-equal list in `entries`.
/// Variants have no short names. CLDR keys them in uppercase (PINYIN), and
/// a code-equal name is that key; ICU4X keys them in lowercase.
fn variant_name(
    data: Option<&VariantDisplayNames>,
    variant: Variant,
    entries: &LazyEntries,
) -> Option<String> {
    from_maps(
        data.map(|d| &d.names),
        None,
        &variant.to_tinystr().to_unvalidated(),
        entries,
        "variant",
        &variant.as_str().to_ascii_uppercase(),
        false,
    )
}

/// The key of a dialect name for `code`: the tag in canonical case
/// (`"en-us"` is `"en-US"`). CLDR's dialect names have a `language-script`
/// or `language-region` key, so any other tag simply has no name. None if
/// `code` does not parse.
fn dialect_key(code: &str) -> Option<String> {
    Some(code.parse::<LanguageIdentifier>().ok()?.to_string())
}

/// The locale display patterns of the language identifier of `locale` (a
/// canonical tag), in CLDR's form: localePattern (`"{0} ({1})"`) and
/// localeSeparator (`"{0}, {1}"`). None if `locale` does not parse.
pub fn locale_patterns(locale: &str) -> Option<[String; 2]> {
    let entries = Entries::new(names_table(), &language_id(locale)?);
    // parse_names checks that und has both.
    Some(["lp", "ls"].map(|key| entries.get(key).unwrap().to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `name(locale, kind, code, style)`, which must accept its input.
    fn get(locale: &str, kind: &str, code: &str, style: &str) -> Option<String> {
        name(locale, kind, code, style)
            .unwrap_or_else(|| panic!("{locale} {kind} {code} {style}: rejected"))
    }

    /// The name, which must exist.
    fn named(locale: &str, kind: &str, code: &str, style: &str) -> String {
        get(locale, kind, code, style)
            .unwrap_or_else(|| panic!("{locale} {kind} {code} {style}: no name"))
    }

    const STYLES: [&str; 3] = ["long", "short", "narrow"];

    #[test]
    fn regions() {
        assert_eq!(named("en", "region", "US", "long"), "United States");
        // From the code-equal list: ICU4X alone falls back to the long name.
        assert_eq!(named("en", "region", "US", "short"), "US");
        assert_eq!(named("en", "region", "US", "narrow"), "US");
        assert_eq!(named("en", "region", "GB", "short"), "UK");
        assert_eq!(named("en", "region", "BA", "short"), "Bosnia");
        assert_eq!(named("de", "region", "UN", "short"), "UN");
        // hi-Latn inherits en-IN's code-equal short US, but ICU4X has its
        // own name, which comes first.
        assert_eq!(named("hi-Latn", "region", "US", "short"), "America");
    }

    #[test]
    fn scripts() {
        assert_eq!(named("en", "script", "Hant", "long"), "Traditional");
        assert_eq!(named("en", "script", "Thai", "long"), "Thai");
        assert_eq!(named("en", "script", "Latn", "long"), "Latin");
    }

    #[test]
    fn languages() {
        assert_eq!(named("en", "language", "en", "long"), "English");
        assert_eq!(named("en", "dialect", "en-US", "long"), "American English");
        assert_eq!(named("en", "dialect", "en-US", "short"), "US English");
        assert_eq!(named("en", "dialect", "en-us", "long"), "American English");
        assert_eq!(named("zh-Hant", "dialect", "zh-Hant", "long"), "繁體中文");
        assert_eq!(named("en", "language", "und", "long"), "Unknown language");
        assert_eq!(named("fr-CA", "language", "fon", "long"), "fon");
        // nn inherits no's code-equal tiv, but ICU4X has nn's own name.
        assert_eq!(named("nn", "language", "tiv", "long"), "tivi");
    }

    #[test]
    fn variants() {
        assert_eq!(
            named("en", "variant", "1996", "long"),
            "German orthography of 1996"
        );
        // Native code returns CLDR's name; JS's composition turns its
        // parentheses into brackets.
        assert_eq!(
            named("ja", "variant", "1996", "long"),
            "ドイツ語正書法(1996)"
        );
        // ICU4X keys variants in lowercase.
        assert_eq!(
            named("en", "variant", "pinyin", "long"),
            "Pinyin romanization"
        );
    }

    #[test]
    fn currencies() {
        assert_eq!(named("en", "currency", "USD", "long"), "US Dollar");
        let listed: Vec<&str> = include_str!("../data/currencies.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .collect();
        // CLDR 49's non-deprecated codes; every one has an English name.
        assert_eq!(listed.len(), 306);
        let mut found = Vec::new();
        for a in b'A'..=b'Z' {
            for b in b'A'..=b'Z' {
                for c in b'A'..=b'Z' {
                    let code = String::from_utf8(vec![a, b, c]).unwrap();
                    if get("en", "currency", &code, "long").is_some() {
                        found.push(code);
                    }
                }
            }
        }
        assert_eq!(found, listed);
    }

    #[test]
    fn calendars() {
        assert_eq!(
            named("en", "calendar", "gregory", "long"),
            "Gregorian Calendar"
        );
        assert_eq!(
            named("en", "calendar", "ethioaa", "long"),
            named("en", "calendar", "ethiopic-amete-alem", "long")
        );
        assert_eq!(
            named("en", "calendar", "islamicc", "long"),
            named("en", "calendar", "islamic-civil", "long")
        );
        assert_eq!(get("en", "calendar", "islamic", "long"), None);
        assert_eq!(get("en", "calendar", "islamic-rgsa", "long"), None);
        assert_eq!(crate::supported::CALENDARS.len(), 16);
        for cal in crate::supported::CALENDARS {
            for style in STYLES {
                named("en", "calendar", cal, style);
            }
        }
    }

    #[test]
    fn fields() {
        let styles = |locale, code| STYLES.map(|s| named(locale, "dateTimeField", code, s));
        assert_eq!(styles("en", "year"), ["year", "yr.", "yr"]);
        assert_eq!(
            styles("en", "weekday"),
            ["day of the week", "day of wk.", "day of wk."]
        );
        assert_eq!(styles("de", "era"), ["Epoche", "Ep.", "E"]);
        assert_eq!(
            named("ar", "dateTimeField", "timeZoneName", "long"),
            "التوقيت"
        );
        assert_eq!(get("en", "dateTimeField", "week", "long"), None);
    }

    #[test]
    fn patterns() {
        assert_eq!(
            locale_patterns("en"),
            Some(["{0} ({1})".to_owned(), "{0}, {1}".to_owned()])
        );
        let [pattern, _] = locale_patterns("zh-Hant").unwrap();
        assert!(
            pattern.contains('\u{FF08}') && pattern.contains('\u{FF09}'),
            "{pattern}"
        );
        assert_eq!(locale_patterns("ar").unwrap()[1], "{0}، {1}");
        assert_eq!(locale_patterns("en_US"), None);
    }

    /// The kinds of `name`, each with a code to look up.
    const KINDS: [(&str, &str); 8] = [
        ("language", "en"),
        ("dialect", "en-US"),
        ("region", "US"),
        ("script", "Latn"),
        ("variant", "1996"),
        ("currency", "USD"),
        ("calendar", "gregory"),
        ("dateTimeField", "year"),
    ];

    /// The names that some locales of locales.txt other than und lack,
    /// each with those locales. CLDR 49 (cldr-localenames-full 49.0.0
    /// territories.json, scripts.json and localeDisplayNames.json,
    /// cldr-numbers-full currencies.json) has no such name for them or
    /// their parents short of root, which has none either. Node (CLDR
    /// 48.0) lacks the same names in the same locales, where it has the
    /// locale (it has none of ady, kbd, quc and suz-Sunu, and has prg
    /// without them); CLDR 49 added lij's calendar name.
    const MISSING: [(&str, &str, &[&str]); 4] = [
        (
            "region",
            "US",
            &[
                "ady", "ady-JO", "ady-TR", "bgc", "bho", "bua", "gaa", "ie", "kbd", "kbd-TR",
                "lij", "lmo", "nds", "nds-NL", "nso", "oc", "oc-ES", "pms", "prg", "quc", "raj",
                "rw", "st", "st-LS", "suz-Sunu", "szl", "tn", "tn-BW", "vmw", "za",
            ],
        ),
        (
            "script",
            "Latn",
            &[
                "ady", "ady-JO", "ady-TR", "bgc", "bho", "bua", "kbd", "kbd-TR", "raj", "suz-Sunu",
            ],
        ),
        (
            "calendar",
            "gregory",
            &[
                "ady", "ady-JO", "ady-TR", "bgc", "bho", "bua", "ckb", "ckb-IR", "csw", "gaa",
                "kbd", "kbd-TR", "ki", "kok-Latn", "lmo", "nds", "nds-NL", "nqo", "nso", "oc",
                "oc-ES", "pms", "prg", "quc", "raj", "rw", "sah", "st", "st-LS", "suz-Sunu", "szl",
                "tn", "tn-BW", "vmw", "za",
            ],
        ),
        (
            "currency",
            "USD",
            &[
                "ady", "ady-JO", "ady-TR", "bgc", "bho", "bua", "ckb", "ckb-IR", "csw", "gaa",
                "ie", "ii", "kbd", "kbd-TR", "kok-Latn", "lij", "lmo", "mt", "nds", "nds-NL",
                "nso", "oc", "oc-ES", "pms", "prg", "quc", "raj", "rw", "st", "st-LS", "suz-Sunu",
                "szl", "tn", "tn-BW", "to", "tyv", "vmw", "za",
            ],
        ),
    ];

    #[test]
    fn every_locale() {
        let mut missing: Vec<Vec<&str>> = vec![Vec::new(); MISSING.len()];
        for locale in crate::locale::test_locales() {
            for (kind, code) in KINDS {
                for style in STYLES {
                    assert!(
                        name(locale, kind, code, style).is_some(),
                        "{locale} {kind} {style}"
                    );
                }
            }
            let [pattern, separator] =
                locale_patterns(locale).unwrap_or_else(|| panic!("{locale}: no patterns"));
            for p in [&pattern, &separator] {
                assert!(p.contains("{0}") && p.contains("{1}"), "{locale}: {p}");
            }
            if locale == "und" {
                continue;
            }
            for (kind, code) in [("language", "en"), ("dateTimeField", "year")] {
                for style in STYLES {
                    named(locale, kind, code, style);
                }
            }
            for (i, (kind, code, _)) in MISSING.iter().enumerate() {
                // Every style has a name, or none has.
                let found = STYLES.map(|style| get(locale, kind, code, style).is_some());
                assert!(found == [found[0]; 3], "{locale} {kind} {code}");
                if !found[0] {
                    missing[i].push(locale);
                }
            }
        }
        for (i, (kind, code, pinned)) in MISSING.iter().enumerate() {
            assert_eq!(missing[i], *pinned, "{kind} {code}");
        }
        // und has only the date-time field names (and the patterns, checked
        // above): CLDR root has no language, dialect, region, script,
        // variant, calendar or currency names, and ICU4X has no root data
        // for any of those names either.
        for (kind, code) in KINDS {
            let found = STYLES.map(|style| get("und", kind, code, style).is_some());
            assert_eq!(found, [kind == "dateTimeField"; 3], "und {kind} {code}");
        }
    }

    #[test]
    fn rejects() {
        assert_eq!(name("en", "lang", "en", "long"), None);
        assert_eq!(name("en", "region", "US", "wide"), None);
        assert_eq!(name("en_US", "region", "US", "long"), None);
        assert_eq!(name("en", "region", "ZZZZ", "long"), Some(None));
        assert_eq!(name("en", "currency", "usd", "long"), Some(None));
        assert_eq!(name("en", "language", "en-US", "long"), Some(None));
        assert_eq!(name("en", "dialect", "en-Latn-US", "long"), Some(None));
        assert_eq!(name("en", "calendar", "ca.gregory", "long"), Some(None));
    }

    /// The parsed `text`, as a table that lives as long as the test binary.
    fn synthetic(text: &'static str) -> &'static NamesTable {
        Box::leak(Box::new(parse_names(text)))
    }

    #[test]
    fn code_equal_variants() {
        // CLDR 49 has no code-equal variant name, so a table of our own
        // stands in. Its eq keys are CLDR's uppercase variant keys.
        let table = synthetic(
            "# A comment line.\n\
             locale und -\n\
             lp\t{0} ({1})\n\
             ls\t{0}, {1}\n\
             eq.variant.long.PINYIN\t\n\
             eq.variant.long.wadegile\t\n",
        );
        let id: LanguageIdentifier = "en".parse().unwrap();
        let variant = |code: &str| {
            let entries = LazyEntries::new(table, &id);
            variant_name(None, Variant::try_from_str(code).unwrap(), &entries)
        };
        // The subtag is uppercased to match, and the name is CLDR's key.
        assert_eq!(variant("pinyin").as_deref(), Some("PINYIN"));
        // A lowercase key never matches.
        assert_eq!(variant("wadegile"), None);
        assert_eq!(variant("1996"), None);
    }

    #[test]
    #[should_panic(expected = "display-names.txt: bad line \"locale und\"")]
    fn malformed_locale_line() {
        parse_names("locale und\nlp\ta\nls\tb\n");
    }

    #[test]
    // An eq key, whose value is empty: only the missing tab is wrong.
    #[should_panic(expected = "display-names.txt: bad line \"eq.region.short.US\"")]
    fn key_line_without_tab() {
        parse_names("locale und -\nlp\ta\nls\tb\neq.region.short.US\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: bad line \"lp\\ta\"")]
    fn key_line_before_locale() {
        parse_names("lp\ta\nlocale und -\nls\tb\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: bad line \"eq.region.short.US\\tUS\"")]
    fn eq_key_with_value() {
        parse_names("locale und -\nlp\ta\nls\tb\neq.region.short.US\tUS\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: bad line \"ca.gregory\\t\"")]
    fn other_key_without_value() {
        parse_names("locale und -\nlp\ta\nls\tb\nca.gregory\t\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: repeated \"lp\\tb\"")]
    fn repeated_key() {
        parse_names("locale und -\nlp\ta\nlp\tb\nls\tb\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: repeated locale und")]
    fn repeated_locale() {
        parse_names("locale und -\nlp\ta\nls\tb\nlocale und -\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: en comes before its parent")]
    fn parent_out_of_order() {
        parse_names("locale en und\nlocale und -\nlp\ta\nls\tb\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: \"locale en -\"")]
    fn root_other_than_und() {
        parse_names("locale en -\nlp\ta\nls\tb\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: no und")]
    fn missing_und() {
        parse_names("# Only a comment.\n");
    }

    #[test]
    #[should_panic(expected = "display-names.txt: und has no patterns")]
    fn missing_patterns() {
        parse_names("locale und -\nlp\t{0} ({1})\n");
    }
}

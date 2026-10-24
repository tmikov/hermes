// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//
// Portions adapted from magrinj/hermes-intl, Copyright (c) 2026 Jérémy Magrin, MIT License; see intl/THIRD_PARTY_NOTICES.md.

//! Locale identifiers: well-formedness, canonicalization (ECMA-402
//! IsStructurallyValidLanguageTag and CanonicalizeUnicodeLocaleId) and the
//! available-locale set.

use icu::locale::subtags::Script;
use icu::locale::{LanguageIdentifier, Locale, LocaleCanonicalizer, LocaleExpander};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// IsWellFormedLanguageTag + CanonicalizeUnicodeLocaleId. `None` = RangeError.
pub fn canonicalize(tag: &str) -> Option<String> {
    // ICU4X also accepts `_` separators; BCP 47 does not.
    if tag.is_empty() || tag.contains('_') {
        return None;
    }
    // ICU4X languages are 2-3 letters; unicode_language_subtag also allows 5-8 ("posix").
    // Canonicalize the rest under "und" and put the language back (no aliases exist for them).
    let first = tag.split('-').next().unwrap_or("");
    let long = (5..=8).contains(&first.len()) && first.bytes().all(|b| b.is_ascii_alphabetic());
    let parsed = if long {
        format!("und{}", &tag[first.len()..])
    } else {
        tag.to_owned()
    };
    let mut loc: Locale = parsed.parse().ok()?;
    // Before the canonicalizer runs, so that it still applies the
    // subdivision aliases to -u-rg and -u-sd.
    keep_true_subtags(&parsed, &mut loc);
    static C: OnceLock<LocaleCanonicalizer> = OnceLock::new();
    C.get_or_init(LocaleCanonicalizer::new_extended)
        .canonicalize(&mut loc);
    canonicalize_keyword_values(&mut loc);
    let out = loc.to_string();
    Some(if long {
        format!("{}{}", first.to_ascii_lowercase(), &out[3..])
    } else {
        out
    })
}

/// Set each keyword value of `loc`'s -u- extension to the value as written
/// in `tag`, the tag `loc` was parsed from. ICU4X's parser drops every
/// "true" subtag of a value, but UTS 35 Annex C removes a value only when
/// the whole value is "true" (canonicalize_keyword_values does that), so
/// en-u-ca-true-foo keeps its value. Collecting subtags into a Value keeps
/// them. A repeated key keeps its first value, as in ICU4X's parse.
fn keep_true_subtags(tag: &str, loc: &mut Locale) {
    use icu::locale::extensions::unicode::{Key, Value};
    use icu::locale::subtags::Subtag;
    // The subtags after the language. The language has 2 or more
    // characters, so the first "u" among them is the -u- singleton, unless
    // a "x" comes first: everything after it is private use.
    let mut subtags = tag.split('-').skip(1);
    if !subtags
        .by_ref()
        .take_while(|s| !s.eq_ignore_ascii_case("x"))
        .any(|s| s.eq_ignore_ascii_case("u"))
    {
        return;
    }
    // The attributes and keywords, up to the next singleton.
    let mut keywords: Vec<(Key, Vec<Subtag>)> = Vec::new();
    for subtag in subtags.take_while(|s| s.len() > 1) {
        if subtag.len() == 2 {
            let Ok(key) = subtag.parse() else { return };
            keywords.push((key, Vec::new()));
        } else if let Some((_, value)) = keywords.last_mut() {
            let Ok(subtag) = subtag.parse() else { return };
            value.push(subtag);
        }
    }
    let mut seen = BTreeSet::new();
    for (key, value) in keywords {
        if seen.insert(key) {
            loc.extensions
                .unicode
                .keywords
                .set(key, value.into_iter().collect::<Value>());
        }
    }
}

/// The value aliases of data/extension-value-aliases.txt (CLDR
/// bcp47/*.xml): (extension, key, alias) to the canonical value, where the
/// extension is "u" for a -u- keyword and "t" for a -t- field.
pub(crate) fn value_aliases(
) -> &'static BTreeMap<(&'static str, &'static str, &'static str), &'static str> {
    static MAP: OnceLock<BTreeMap<(&str, &str, &str), &str>> = OnceLock::new();
    MAP.get_or_init(|| {
        include_str!("../data/extension-value-aliases.txt")
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(|l| match l.split(' ').collect::<Vec<_>>()[..] {
                [ext, key, alias, value] => ((ext, key, alias), value),
                _ => panic!("data/extension-value-aliases.txt: bad line {l:?}"),
            })
            .collect()
    })
}

/// The canonical value of `value`, a value of `key` in extension `ext`
/// ("u" or "t"), if `value` is an alias.
fn value_alias(ext: &str, key: &str, value: &str) -> Option<&'static str> {
    value_aliases().get(&(ext, key, value)).copied()
}

/// The keyword and field value half of UTS 35 Annex C, which ICU4X leaves
/// out: a -u- keyword value or -t- field value that CLDR lists as an alias
/// of its key (value_aliases) is replaced by its canonical value, and then a
/// -u- value that is "true" is removed. Only a whole value is replaced.
/// CLDR's aliases include "yes" for "true" for the boolean collation keys
/// (kb, kc, kh, kk, kn), so en-u-kn-yes becomes en-u-kn.
fn canonicalize_keyword_values(loc: &mut Locale) {
    use icu::locale::extensions::{transform, unicode};
    let keywords = &mut loc.extensions.unicode.keywords;
    let fixes: Vec<(unicode::Key, String)> = keywords
        .iter()
        .filter_map(|(key, value)| {
            let value = value.to_string();
            let new = value_alias("u", key.as_str(), &value).unwrap_or(&value);
            (new != value || new == "true").then(|| (*key, new.to_owned()))
        })
        .collect();
    for (key, value) in fixes {
        keywords.set(
            key,
            if value == "true" {
                unicode::Value::default()
            } else {
                // Collected, not parsed, so that a "true" subtag stays.
                value.split('-').filter_map(|s| s.parse().ok()).collect()
            },
        );
    }
    let fields = &mut loc.extensions.transform.fields;
    for (&(ext, key, alias), value) in value_aliases() {
        if ext != "t" {
            continue;
        }
        let Ok(key) = key.parse::<transform::Key>() else {
            continue;
        };
        if fields.get(&key).is_some_and(|v| v.to_string() == alias) {
            if let Ok(value) = value.parse() {
                fields.set(key, value);
            }
        }
    }
}

fn expander() -> &'static LocaleExpander {
    static E: OnceLock<LocaleExpander> = OnceLock::new();
    E.get_or_init(LocaleExpander::new_extended)
}

/// [[AvailableLocales]] (ECMA-402 9.1), canonical and sorted, computed on
/// first use. It is the closure of data/locales.txt (the locales the compiled
/// data covers) under two rules:
/// - (a) every canonical tag that maximizes to the same tag as an entry is
///   available, because ICU4X resolves it to that entry's data: CLDR stores
///   "en-US" as "en", "zh-Hant-TW" as "zh-Hant", "sr-Cyrl-RS" as "sr";
/// - (b) 9.1 requires a less narrow tag for every tag with more than one
///   subtag, and the language-region form of every language-script-region
///   tag, so every prefix (dropping trailing subtags, as
///   LookupMatchingLocaleByPrefix does) and every L-R form of an available
///   tag is available too.
pub fn available() -> &'static [String] {
    static LIST: OnceLock<Vec<String>> = OnceLock::new();
    LIST.get_or_init(|| availability().keys().cloned().collect())
}

/// Each available tag, with the scripts of the data/locales.txt entries
/// that made it available (their likely scripts). A tag that rule (a) of
/// available() reaches has data in its own likely script; one that only
/// rule (b) reaches may not: "ff" (likely Latin) is available only as a
/// prefix of the Adlam entries, and ICU4X has no Latin "ff" data.
fn availability() -> &'static BTreeMap<String, BTreeSet<Option<Script>>> {
    static MAP: OnceLock<BTreeMap<String, BTreeSet<Option<Script>>>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = BTreeMap::new();
        for line in include_str!("../data/locales.txt").lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let entry: LanguageIdentifier = canonicalize(line)
                .and_then(|t| t.parse().ok())
                .unwrap_or_else(|| {
                    panic!("data/locales.txt: {line:?} is not a plain language tag")
                });
            // (a): the entry's maximized form with its script, its region,
            // or both left out, where that maximizes back to the same tag.
            let mut max = entry.clone();
            expander().maximize(&mut max);
            add_with_fallbacks(&mut map, &entry, max.script);
            for script in [max.script, None] {
                for region in [max.region, None] {
                    let mut form = max.clone();
                    form.script = script;
                    form.region = region;
                    let mut form_max = form.clone();
                    expander().maximize(&mut form_max);
                    if form_max == max {
                        add_with_fallbacks(&mut map, &form, max.script);
                    }
                }
            }
        }
        map
    })
}

/// Add `id` and, by rule (b) of available(), its less narrow forms, as
/// reached from an entry whose data is in `script`.
fn add_with_fallbacks(
    map: &mut BTreeMap<String, BTreeSet<Option<Script>>>,
    id: &LanguageIdentifier,
    script: Option<Script>,
) {
    let tag = id.to_string();
    if !map.entry(tag.clone()).or_default().insert(script) {
        return;
    }
    if let (Some(_), Some(region)) = (id.script, id.region) {
        add_with_fallbacks(
            map,
            &LanguageIdentifier::from((id.language, None, Some(region))),
            script,
        );
    }
    // With no extensions every subtag after the language is a script, region
    // or variant, so dropping the last one leaves a well-formed tag.
    if let Some(pos) = tag.rfind('-') {
        let prefix: LanguageIdentifier = tag[..pos]
            .parse()
            .unwrap_or_else(|_| panic!("{tag:?}: prefix is not well-formed"));
        add_with_fallbacks(map, &prefix, script);
    }
}

/// Whether `tag` (canonical, without extensions) is in [[AvailableLocales]].
pub fn is_available(tag: &str) -> bool {
    available()
        .binary_search_by(|a| a.as_str().cmp(tag))
        .is_ok()
}

/// The script `tag` is written in: its script subtag, or the likely one.
pub fn likely_script(tag: &str) -> Option<Script> {
    let mut id = tag.parse::<Locale>().ok()?.id;
    expander().maximize(&mut id);
    id.script
}

/// LookupMatchingLocaleByPrefix (ECMA-402 9.2.3) for one canonical tag,
/// with its extensions (-u-, -t-, -x- and the rest) dropped first: the
/// longest available prefix of it, or None. Because the extensions are gone
/// there is no dangling singleton to drop while truncating.
pub fn lookup(tag: &str) -> Option<String> {
    let loc: Locale = tag.parse().ok()?;
    let mut prefix = loc.id.to_string();
    loop {
        if is_available(&prefix) {
            return Some(prefix);
        }
        let pos = prefix.rfind('-')?;
        prefix.truncate(pos);
    }
}

/// A well-formed BCP 47 tag to a DefaultLocale (ECMA-402): a canonical tag
/// with no extensions that is in [[AvailableLocales]]. The tag is
/// canonicalized, its extensions dropped, and it is reduced to its longest
/// available prefix (lookup), but never to one written in another script
/// than the tag, and never to one without data in its own script:
/// tt-Latn-RU would truncate to "tt", which is Cyrillic, and "ff" (likely
/// Latin) is available only through the Adlam entries. None if it is not
/// well-formed or no prefix is usable. Both host detection and the explicit
/// IntlDefaultLocales go through this, so every default is one the plugin
/// can serve.
pub fn resolve_default(tag: &str) -> Option<String> {
    let tag = canonicalize(tag)?;
    let found = lookup(&tag)?;
    let script = likely_script(&found);
    if script != likely_script(&tag) || !availability()[&found].contains(&script) {
        return None;
    }
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_defaults() {
        let cases = [
            ("en-US-u-ca-buddhist", Some("en-US")),
            ("EN-us-t-de-x-foo", Some("en-US")),
            ("zh-TW", Some("zh-TW")),
            ("zh-Hant-US", Some("zh-Hant")),
            ("iw", Some("he")),
            ("en-AQ", Some("en")),
            // The likely script changes on truncation (Latin to Cyrillic).
            ("tt-Latn-RU", None),
            // Available, but the data is only in another script than the
            // likely one: Adlam, not Latin, for ff; Latin, not Arabic, for
            // bal.
            ("ff", None),
            ("ff-BF", None),
            ("ff-Latn-SN", None),
            ("bal", None),
            // An L-R form of ku-Latn-IQ, but likely Arabic.
            ("ku-IQ", None),
            // The same languages in the scripts the data has.
            ("ff-Adlm-BF", Some("ff-Adlm-BF")),
            ("ff-Adlm-u-nu-adlm", Some("ff-Adlm")),
            ("bal-Latn-PK", Some("bal-Latn-PK")),
            // "sr" is reached from sr-Latn too, but it is also an entry.
            ("sr", Some("sr")),
            ("sr-RS", Some("sr-RS")),
            ("tlh", None),
            ("tlh-US", None),
            ("en_US", None),
            ("", None),
        ];
        for (tag, want) in cases {
            assert_eq!(resolve_default(tag).as_deref(), want, "{tag:?}");
        }
    }

    #[test]
    fn canonicalizes() {
        assert_eq!(canonicalize("EN-us").as_deref(), Some("en-US"));
        assert_eq!(canonicalize("iw").as_deref(), Some("he"));
        assert_eq!(
            canonicalize("en-u-ca-islamicc").as_deref(),
            Some("en-u-ca-islamic-civil")
        );
        // UTS 35 Annex C removes a keyword value only when the whole value
        // is "true"; a "true" subtag of a longer value stays.
        for (tag, want) in [
            ("en-u-kn-true", "en-u-kn"),
            ("EN-U-KN-TRUE", "en-u-kn"),
            ("en-u-ca-true-foo", "en-u-ca-true-foo"),
            ("en-u-ca-foo-true", "en-u-ca-foo-true"),
            ("en-u-co-true-true", "en-u-co-true-true"),
            ("en-u-CA-True-Foo", "en-u-ca-true-foo"),
            // The first occurrence of a repeated key wins; attributes are
            // sorted and deduplicated.
            ("en-u-ca-true-foo-ca-bar", "en-u-ca-true-foo"),
            ("en-u-ca-bar-ca-true-foo", "en-u-ca-bar"),
            ("en-u-bbb-aaa-bbb-ca-true-foo", "en-u-aaa-bbb-ca-true-foo"),
            // Keys are sorted, each with its own value.
            (
                "en-u-nu-true-arab-ca-true-foo-kn",
                "en-u-ca-true-foo-kn-nu-true-arab",
            ),
            // Private use is not an extension.
            ("en-x-u-ca-true-foo", "en-x-u-ca-true-foo"),
            (
                "en-u-ca-true-foo-x-u-ca-true",
                "en-u-ca-true-foo-x-u-ca-true",
            ),
            // Next to the other extensions, and with a 5-8 letter language.
            ("en-u-ca-true-foo-t-de-a-bc", "en-a-bc-t-de-u-ca-true-foo"),
            ("posix-u-ca-true-foo", "posix-u-ca-true-foo"),
            // ICU4X still applies the subdivision aliases.
            ("en-u-sd-cn11", "en-u-sd-cnbj"),
        ] {
            assert_eq!(canonicalize(tag).as_deref(), Some(want), "{tag:?}");
        }
        assert_eq!(
            canonicalize("de-u-co-phonebk-ka-shifted").as_deref(),
            Some("de-u-co-phonebk-ka-shifted")
        );
        assert_eq!(canonicalize("posix").as_deref(), Some("posix"));
    }

    #[test]
    fn canonicalizes_value_aliases() {
        for (tag, want) in [
            // Deprecated time zones with a preferred value (CLDR 48.2).
            ("en-u-tz-aukns", "en-u-tz-auhba"),
            ("en-u-tz-camtr", "en-u-tz-cator"),
            ("en-u-tz-caffs", "en-u-tz-cawnp"),
            ("en-u-tz-aqams", "en-u-tz-aqmcm"),
            ("en-u-tz-cnckg", "en-u-tz-cnsha"),
            // Well-formed IANA aliases of a zone; uppercase input.
            ("en-u-tz-EIRE", "en-u-tz-iedub"),
            ("en-u-tz-gmt0", "en-u-tz-gmt"),
            // An alias of a deprecated zone goes on to its preferred value.
            ("en-u-tz-est5edt", "en-u-tz-usnyc"),
            ("en-u-ms-imperial", "en-u-ms-uksystem"),
            ("en-u-ks-primary", "en-u-ks-level1"),
            ("en-u-ca-ethiopic-amete-alem", "en-u-ca-ethioaa"),
            ("en-u-ca-islamicc", "en-u-ca-islamic-civil"),
            ("en-t-m0-names", "en-t-m0-prprname"),
            ("en-t-m0-beta-metsehaf", "en-t-m0-betamets"),
            ("en-t-d0-name", "en-t-d0-charname"),
            // "yes" is "true" for the boolean collation keys only, and a
            // value that is "true" after replacement is removed.
            ("en-u-kn-yes", "en-u-kn"),
            ("en-u-kb-yes-kc-yes", "en-u-kb-kc"),
            ("en-u-kf-yes", "en-u-kf-yes"),
            ("en-u-co-yes", "en-u-co-yes"),
            // Only a whole value is replaced, and only for its own key and
            // extension.
            ("en-u-tz-cuba-foo", "en-u-tz-cuba-foo"),
            ("en-u-ca-cuba", "en-u-ca-cuba"),
            ("en-u-tz-names", "en-u-tz-names"),
            ("en-t-m0-imperial", "en-t-m0-imperial"),
            // Names stay, even when another type lists them as an alias.
            ("en-u-ca-islamic-civil", "en-u-ca-islamic-civil"),
            // Deprecated values with no preferred value stay.
            ("en-u-co-direct", "en-u-co-direct"),
            ("en-u-hc-c12", "en-u-hc-c12"),
        ] {
            assert_eq!(canonicalize(tag).as_deref(), Some(want), "{tag:?}");
        }
    }

    #[test]
    fn every_value_alias_canonicalizes_to_its_value() {
        assert!(value_aliases().len() > 60);
        for (&(ext, key, alias), &value) in value_aliases() {
            let want = if ext == "u" && value == "true" {
                format!("und-u-{key}")
            } else {
                format!("und-{ext}-{key}-{value}")
            };
            let tag = format!("und-{ext}-{key}-{alias}");
            assert_eq!(canonicalize(&tag), Some(want), "{tag:?}");
        }
    }

    #[test]
    fn rejects_ill_formed_tags() {
        for bad in [
            "",
            "en_US",
            "en-",
            "-en",
            "x-private",
            "en-US-u",
            "a",
            "en--US",
            "toolonglanguage",
        ] {
            assert_eq!(canonicalize(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn availability() {
        assert!(is_available("en"));
        assert!(is_available("en-US"));
        assert!(is_available("sr-Cyrl-RS"));
        assert!(!is_available("en-AQ"));
        assert!(!is_available("tlh"));
        assert!(!is_available("en-u-ca-gregory"));
        // Rule (a): same maximized form as an entry ("zh-Hant", "sr-Latn",
        // "zh", "ku", "ff-Adlm-BF"...).
        for tag in [
            "zh-TW",
            "zh-Hant-TW",
            "zh-Hant",
            "zh-Hans",
            "sr-Latn-RS",
            "uz-Cyrl-UZ",
        ] {
            assert!(is_available(tag), "{tag}");
        }
        // Rule (b): prefixes and L-R forms of available tags
        // ("sr-Cyrl-ME", "ff-Adlm-BF", "ku-Latn-IQ", "bal-Latn").
        for tag in [
            "sr-Cyrl", "ff-BF", "ku-IQ", "ku-Latn", "yue-CN", "yue-Hant", "bal",
        ] {
            assert!(is_available(tag), "{tag}");
        }
        // Neither rule reaches these.
        for tag in ["zh-Hant-US", "sr-Latn-US", "ff-Latn-JP"] {
            assert!(!is_available(tag), "{tag}");
        }
    }

    #[test]
    fn lookup_truncates_to_an_available_prefix() {
        assert_eq!(lookup("en-US-u-ca-buddhist").as_deref(), Some("en-US"));
        assert_eq!(lookup("en-US-x-foo").as_deref(), Some("en-US"));
        assert_eq!(lookup("zh-Hant-US").as_deref(), Some("zh-Hant"));
        assert_eq!(lookup("de-CH-1996").as_deref(), Some("de-CH"));
        assert_eq!(lookup("tlh-US"), None);
    }

    #[test]
    fn available_list_is_canonical() {
        assert!(available().len() > 400);
        for tag in available() {
            assert_eq!(canonicalize(tag).as_deref(), Some(tag.as_str()));
        }
    }
}

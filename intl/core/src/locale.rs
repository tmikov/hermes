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

/// The -u- value half of UTS 35 canonicalization, which ICU4X leaves out: "yes" means "true",
/// "true" is dropped, and deprecated values map to their preferred ones (CLDR bcp47/*.xml).
// The aliases CLDR lists for ca, ks, ms and the common tz ones; extend from CLDR as needed.
fn canonicalize_keyword_values(loc: &mut Locale) {
    use icu::locale::extensions::unicode::Value;
    const ALIASES: &[(&str, &str, &str)] = &[
        ("ca", "ethiopic-amete-alem", "ethioaa"),
        ("ca", "islamicc", "islamic-civil"),
        ("ks", "primary", "level1"),
        ("ks", "secondary", "level2"),
        ("ks", "tertiary", "level3"),
        ("ks", "quarternary", "level4"),
        ("ks", "quaternary", "level4"),
        ("ks", "identical", "identic"),
        ("ms", "imperial", "uksystem"),
        ("tz", "aqams", "nzakl"),
        ("tz", "cnckg", "cnsha"),
        ("tz", "cnhrb", "cnsha"),
        ("tz", "cnkhg", "cnurc"),
        ("tz", "cuba", "cuhav"),
        ("tz", "egypt", "egcai"),
        ("tz", "eire", "iedub"),
        ("tz", "est", "papty"),
        ("tz", "gmt0", "gmt"),
        ("tz", "hongkong", "hkhkg"),
        ("tz", "hst", "utcw10"),
        ("tz", "iceland", "isrey"),
        ("tz", "iran", "irthr"),
        ("tz", "israel", "jeruslm"),
        ("tz", "jamaica", "jmkin"),
        ("tz", "japan", "jptyo"),
        ("tz", "libya", "lytip"),
        ("tz", "mst", "utcw07"),
        ("tz", "navajo", "usden"),
        ("tz", "poland", "plwaw"),
        ("tz", "portugal", "ptlis"),
        ("tz", "prc", "cnsha"),
        ("tz", "roc", "twtpe"),
        ("tz", "rok", "krsel"),
        ("tz", "turkey", "trist"),
        ("tz", "uct", "utc"),
        ("tz", "usnavajo", "usden"),
        ("tz", "zulu", "utc"),
    ];
    let kw = &mut loc.extensions.unicode.keywords;
    let fixes: Vec<_> = kw
        .iter()
        .filter_map(|(k, v)| {
            let v = v.to_string();
            let key = k.as_str();
            // "yes" is an alias of "true" only for the boolean collation keys.
            let boolean = matches!(key, "kb" | "kc" | "kh" | "kk" | "kn");
            let new = if v == "true" || (boolean && v == "yes") {
                String::new()
            } else {
                ALIASES
                    .iter()
                    .find(|a| a.0 == key && a.1 == v)
                    .map(|a| a.2.to_owned())?
            };
            Some((*k, new))
        })
        .collect();
    for (k, v) in fixes {
        kw.set(
            k,
            if v.is_empty() {
                Value::default()
            } else {
                v.parse().unwrap_or_default()
            },
        );
    }
    // The one -t- field alias CLDR defines (bcp47/transform.xml).
    if let Ok(m0) = "m0".parse() {
        let fields = &mut loc.extensions.transform.fields;
        if fields.get(&m0).is_some_and(|v| v.to_string() == "names") {
            if let Ok(v) = "prprname".parse() {
                fields.set(m0, v);
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
        assert_eq!(canonicalize("en-u-kn-true").as_deref(), Some("en-u-kn"));
        assert_eq!(
            canonicalize("de-u-co-phonebk-ka-shifted").as_deref(),
            Some("de-u-co-phonebk-ka-shifted")
        );
        assert_eq!(canonicalize("posix").as_deref(), Some("posix"));
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

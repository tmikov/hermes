// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.
//
// Portions adapted from magrinj/hermes-intl, Copyright (c) 2026 Jérémy Magrin, MIT License; see intl/THIRD_PARTY_NOTICES.md.

//! Locale identifiers: well-formedness, canonicalization (ECMA-402
//! IsStructurallyValidLanguageTag and CanonicalizeUnicodeLocaleId) and the
//! available-locale set.

use icu::locale::subtags::{language, region, script, Language, Script};
use icu::locale::{LanguageIdentifier, Locale, LocaleCanonicalizer, LocaleExpander};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

/// Whether `subtag` is a unicode_language_subtag that ICU4X does not accept:
/// ICU4X languages are 2-3 letters, UTS 35 also allows 5-8 ("posix").
fn is_long_language(subtag: &str) -> bool {
    (5..=8).contains(&subtag.len()) && subtag.bytes().all(|b| b.is_ascii_alphabetic())
}

/// The byte range in `tag` of the subtag after its -t- singleton (the
/// language of its tlang, or its first tfield key), if it has a -t-
/// extension. A one-character subtag after the language is always a
/// singleton until -x-, after which every subtag is private use.
fn transform_first_subtag(tag: &str) -> Option<std::ops::Range<usize>> {
    let mut start = 0;
    let mut after_t = false;
    for (i, subtag) in tag.split('-').enumerate() {
        let range = start..start + subtag.len();
        start = range.end + 1;
        if after_t {
            return Some(range);
        }
        if i > 0 && subtag.eq_ignore_ascii_case("x") {
            return None;
        }
        after_t = i > 0 && subtag.eq_ignore_ascii_case("t");
    }
    None
}

/// `tag` with the language of its -t- tlang replaced by "und" if that
/// language has 5-8 letters (`tlang` begins with a unicode_language_subtag),
/// and the replaced language, lowercased.
fn hide_long_tlang(tag: &str) -> (String, Option<String>) {
    match transform_first_subtag(tag) {
        Some(r) if is_long_language(&tag[r.clone()]) => (
            format!("{}und{}", &tag[..r.start], &tag[r.end..]),
            Some(tag[r].to_ascii_lowercase()),
        ),
        _ => (tag.to_owned(), None),
    }
}

/// Undo hide_long_tlang on `out`, a canonical tag made from the hidden tag,
/// whose tlang language is still the "und" placeholder: no alias has a 5-8
/// letter language, and none turns "und" into another language. None if
/// the placeholder is gone, which would be a bug.
fn restore_long_tlang(out: String, tlang: Option<String>) -> Option<String> {
    let Some(language) = tlang else {
        return Some(out);
    };
    let r = transform_first_subtag(&out)?;
    if &out[r.clone()] != "und" {
        return None;
    }
    Some(format!("{}{}{}", &out[..r.start], language, &out[r.end..]))
}

/// IsWellFormedLanguageTag + CanonicalizeUnicodeLocaleId. `None` = RangeError.
pub fn canonicalize(tag: &str) -> Option<String> {
    // ICU4X also accepts `_` separators; BCP 47 does not.
    if tag.is_empty() || tag.contains('_') {
        return None;
    }
    // ICU4X does not accept a 5-8 letter language ("posix"), as the language
    // or as the language of a -t- tlang. Canonicalize the tag with "und" in
    // its place and put the language back (no aliases exist for them).
    let (tag, tlang) = hide_long_tlang(tag);
    let tlang_hidden = tlang.is_some();
    let first = tag.split('-').next().unwrap_or("");
    let long = is_long_language(first);
    let parsed = if long {
        format!("und{}", &tag[first.len()..])
    } else {
        tag.clone()
    };
    let mut loc: Locale = parsed.parse().ok()?;
    static C: OnceLock<LocaleCanonicalizer> = OnceLock::new();
    let canonicalizer = C.get_or_init(LocaleCanonicalizer::new_extended);
    canonicalizer.canonicalize(&mut loc);
    // UTS 35 Annex C, "Processing LocaleIds": "Replace any tlang languageId
    // value by its canonicalization." ICU4X applies only the language
    // aliases to a tlang (en-t-iw is en-t-he), so canonicalize the tlang
    // again as a languageId of its own, which applies the script, region
    // and variant aliases too (en-t-ja-dd is en-t-ja-de).
    if let Some(tlang) = loc.extensions.transform.lang.take() {
        let mut id = Locale::from(tlang);
        canonicalizer.canonicalize(&mut id);
        // A hidden 5-8 letter language is a real language, not "und": an
        // alias whose type has no language must not give it one (Annex C,
        // "Replacement", fills an empty field only). The aliases checked
        // (und-arevela, und-saaho) leave "und" alone; this keeps the
        // placeholder restore_long_tlang needs in any case.
        if tlang_hidden {
            id.id.language = Language::UNKNOWN;
        }
        loc.extensions.transform.lang = Some(id.id);
    }
    canonicalize_keyword_values(&mut loc);
    let out = loc.to_string();
    let out = if long {
        format!("{}{}", first.to_ascii_lowercase(), &out[3..])
    } else {
        out
    };
    restore_long_tlang(out, tlang)
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

/// `tag` split before its first singleton: its unicode_language_id, and
/// its extensions and private use (empty, or starting with '-'). Every
/// subtag of a language identifier but the first has 2 or more characters,
/// so the first one-character subtag after the first subtag is a
/// singleton.
fn split_language_id(tag: &str) -> (&str, &str) {
    // The end of the subtags seen so far.
    let mut end = 0;
    for (i, subtag) in tag.split('-').enumerate() {
        if i > 0 && subtag.len() == 1 {
            return tag.split_at(end);
        }
        end += usize::from(i > 0) + subtag.len();
    }
    (tag, "")
}

/// The unicode_language_id of `tag`, parsed with its extensions and private
/// use left out. Operations on the language, script, region and variants
/// use this, never a parse of the whole tag: ICU4X rejects tags that are
/// well-formed (a 5-8 letter tlang language) and changes others (it drops
/// "true" subtags of -u- values). None if the language identifier does not
/// parse, as for a 5-8 letter language.
pub(crate) fn language_id(tag: &str) -> Option<LanguageIdentifier> {
    split_language_id(tag).0.parse().ok()
}

/// Which direction `likely_subtags` goes.
enum Likely {
    Add,
    Remove,
}

/// Add Likely Subtags and Remove Likely Subtags (ECMA-402 6.2.4 and UTS 35
/// Likely Subtags) on a language tag. `None` if `tag` is not well-formed.
/// Only the language, script and region change; variants (already in
/// canonical order), extensions and private use stay as they are.
fn likely_subtags(tag: &str, dir: Likely) -> Option<String> {
    let canonical = canonicalize(tag)?;
    let (id, rest) = split_language_id(&canonical);
    // ICU4X languages are 2-3 letters. A 5-8 letter language is never "und",
    // so Lookup has no likely subtags for it: leave the tag alone.
    if is_long_language(id.split('-').next().unwrap_or("")) {
        return Some(canonical);
    }
    // Only the language identifier changes, so only it is parsed, and the
    // canonical extensions and private use are put back as they are.
    // ICU4X could not round-trip them: it does not parse a 5-8 letter tlang
    // language, and its parser drops every "true" subtag of a -u- value.
    let mut id: LanguageIdentifier = id.parse().ok()?;
    // UTS 35 Remove Likely Subtags starts with Add Likely Subtags (its step
    // 1), so both directions maximize first. ICU4X's minimize maximizes
    // again, which leaves a maximal tag unchanged.
    maximize(&mut id);
    if let Likely::Remove = dir {
        expander().minimize(&mut id);
    }
    Some(format!("{}{rest}", canonicalize(&id.to_string())?))
}

/// UTS 35 Add Likely Subtags on a language identifier.
fn maximize(id: &mut LanguageIdentifier) {
    expander().maximize(id);
    // ICU4X leaves a tag whose language is still "und" unchanged when its
    // tables have no entry for the script or region (CLDR gives bare "und"
    // en-Latn-US and "und-150" en-Latn-150). UTS 35 Lookup then ends at the
    // "und" entry, en_Latn_US, whose fields fill whatever is still missing.
    if id.language.is_unknown() {
        id.language = language!("en");
        id.script.get_or_insert(script!("Latn"));
        id.region.get_or_insert(region!("US"));
    }
}

/// Add Likely Subtags of a canonical tag, or `None` if it does not parse.
pub fn add_likely_subtags(tag: &str) -> Option<String> {
    likely_subtags(tag, Likely::Add)
}

/// Remove Likely Subtags of a canonical tag, or `None` if it does not parse.
/// The region is kept in preference to the script, as UTS 35 does.
pub fn remove_likely_subtags(tag: &str) -> Option<String> {
    likely_subtags(tag, Likely::Remove)
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
    let mut id = language_id(tag)?;
    expander().maximize(&mut id);
    id.script
}

/// LookupMatchingLocaleByPrefix (ECMA-402 9.2.3) for one canonical tag,
/// with its extensions (-u-, -t-, -x- and the rest) dropped first: the
/// longest available prefix of it, or None. Because the extensions are gone
/// there is no dangling singleton to drop while truncating.
pub fn lookup(tag: &str) -> Option<String> {
    // The language identifier of a canonical tag is canonical, so it is
    // truncated as it is, without a parse.
    let mut prefix = split_language_id(tag).0.to_owned();
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
            // Only the language identifier matters: a 5-8 letter tlang
            // language, which ICU4X does not parse, changes nothing.
            ("en-US-t-posix", Some("en-US")),
            ("de-t-abcdefgh-latn-u-ca-buddhist", Some("de")),
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

    const LIKELY_EXTRAS: [&str; 7] = [
        "",
        "-fonipa",
        "-a-not-assigned",
        "-u-attr",
        "-u-co",
        "-u-co-phonebk",
        "-x-private",
    ];

    // test262 intl402/Locale/likely-subtags.js testDataMaximal.
    const MAXIMAL: [(&str, &str); 15] = [
        ("en", "en-Latn-US"),
        ("en-Latn", "en-Latn-US"),
        ("en-Shaw", "en-Shaw-GB"),
        ("en-Arab", "en-Arab-US"),
        ("en-US", "en-Latn-US"),
        ("en-GB", "en-Latn-GB"),
        ("en-FR", "en-Latn-FR"),
        ("it-Kana-CA", "it-Kana-CA"),
        ("und", "en-Latn-US"),
        ("und-Thai", "th-Thai-TH"),
        ("und-419", "es-Latn-419"),
        ("und-150", "en-Latn-150"),
        ("und-AT", "de-Latn-AT"),
        ("und-Cyrl-RO", "bg-Cyrl-RO"),
        ("und-AQ", "en-Latn-AQ"),
    ];

    // test262 intl402/Locale/likely-subtags.js testDataMinimal.
    const MINIMAL: [(&str, &str); 17] = [
        ("en", "en"),
        ("en-Latn", "en"),
        ("ar-Arab", "ar"),
        ("en-US", "en"),
        ("en-GB", "en-GB"),
        ("en-Latn-US", "en"),
        ("en-Shaw-GB", "en-Shaw"),
        ("en-Arab-US", "en-Arab"),
        ("en-Latn-GB", "en-GB"),
        ("en-Latn-FR", "en-FR"),
        ("it-Kana-CA", "it-Kana-CA"),
        ("th-Thai-TH", "th"),
        ("es-Latn-419", "es-419"),
        ("ru-Cyrl-RU", "ru"),
        ("de-Latn-AT", "de-AT"),
        ("bg-Cyrl-RO", "bg-RO"),
        ("und-Latn-AQ", "en-AQ"),
    ];

    // test262 intl402/Locale/prototype/minimize/
    // removing-likely-subtags-first-adds-likely-subtags.js testDataMinimal.
    const REMOVE_ADDS_FIRST: [(&str, &str); 11] = [
        ("und", "en"),
        ("und-Thai", "th"),
        ("und-419", "es-419"),
        ("und-150", "en-150"),
        ("und-AT", "de-AT"),
        ("aae-Latn-IT", "aae"),
        ("aae-Thai-CO", "aae-Thai-CO"),
        ("und-CW", "pap"),
        ("und-US", "en"),
        ("zh-Hant", "zh-TW"),
        ("zh-Hani", "zh-Hani"),
    ];

    #[test]
    fn adds_likely_subtags() {
        for (tag, want) in MAXIMAL {
            assert_eq!(add_likely_subtags(want).as_deref(), Some(want), "{want:?}");
            for extra in LIKELY_EXTRAS {
                let (input, want) = (format!("{tag}{extra}"), format!("{want}{extra}"));
                assert_eq!(
                    add_likely_subtags(&input).as_deref(),
                    Some(want.as_str()),
                    "{input:?}"
                );
            }
        }
        let cases = [
            // The "und" entry fills both fields, only the script (region
            // given), or only the region (script given, no data for it).
            ("und", "en-Latn-US"),
            ("und-150", "en-Latn-150"),
            ("und-Qaaa", "en-Qaaa-US"),
            ("posix", "posix"),
            ("xtg", "xtg"),
            ("aa-x-private", "aa-Latn-ET-x-private"),
            ("de-u-kf", "de-Latn-DE-u-kf"),
            ("sl-rozaj-biske-1994", "sl-Latn-SI-1994-biske-rozaj"),
            ("en-t-ja-x-a", "en-Latn-US-t-ja-x-a"),
            // A 5-8 letter tlang language is kept, and the rest of the tlang
            // is canonicalized as for any tlang.
            ("en-t-posix", "en-Latn-US-t-posix"),
            ("fr-t-abcdefgh-latn", "fr-Latn-FR-t-abcdefgh-latn"),
            (
                "de-t-Posix-AT-m0-names",
                "de-Latn-DE-t-posix-at-m0-prprname",
            ),
            ("posix-t-abcdef", "posix-t-abcdef"),
        ];
        for (tag, want) in cases {
            assert_eq!(add_likely_subtags(tag).as_deref(), Some(want), "{tag:?}");
        }
    }

    #[test]
    fn removes_likely_subtags() {
        for (tag, want) in MINIMAL {
            assert_eq!(
                remove_likely_subtags(want).as_deref(),
                Some(want),
                "{want:?}"
            );
            for extra in LIKELY_EXTRAS {
                let (input, want) = (format!("{tag}{extra}"), format!("{want}{extra}"));
                assert_eq!(
                    remove_likely_subtags(&input).as_deref(),
                    Some(want.as_str()),
                    "{input:?}"
                );
            }
        }
        let cases = [
            ("posix-u-ca-buddhist", "posix-u-ca-buddhist"),
            ("en-Latn-US-t-ja-x-a", "en-t-ja-x-a"),
            ("zh-Hant-TW", "zh-TW"),
            // Remove Likely Subtags starts with Add Likely Subtags, so the
            // "und" entry fills what ICU4X's tables leave missing.
            ("und-Qaaa", "en-Qaaa"),
            ("und-150-u-ca-buddhist", "en-150-u-ca-buddhist"),
            ("en-Latn-US-t-posix", "en-t-posix"),
            ("fr-t-abcdefgh-latn", "fr-t-abcdefgh-latn"),
            ("de-Latn-DE-t-posix-AT", "de-t-posix-at"),
        ];
        for (tag, want) in cases {
            assert_eq!(remove_likely_subtags(tag).as_deref(), Some(want), "{tag:?}");
        }
        for (tag, want) in REMOVE_ADDS_FIRST {
            assert_eq!(remove_likely_subtags(tag).as_deref(), Some(want), "{tag:?}");
            assert_eq!(
                remove_likely_subtags(want).as_deref(),
                Some(want),
                "{want:?}"
            );
        }
    }

    #[test]
    fn likely_subtags_round_trip() {
        for (_, minimal) in MINIMAL {
            let max = add_likely_subtags(minimal).unwrap();
            assert_eq!(remove_likely_subtags(&max).as_deref(), Some(minimal));
        }
    }

    #[test]
    fn likely_subtags_reject_unparsable() {
        for bad in ["", "en_US"] {
            assert_eq!(add_likely_subtags(bad), None, "{bad:?}");
            assert_eq!(remove_likely_subtags(bad), None, "{bad:?}");
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
        // A tlang starts with a unicode_language_subtag, which may have 5-8
        // letters; the rest of the tlang is canonicalized as for any tlang
        // (lowercased, variants sorted, aliases applied).
        for (tag, want) in [
            ("en-t-posix", "en-t-posix"),
            ("fr-t-abcdefgh-latn", "fr-t-abcdefgh-latn"),
            ("EN-T-POSIX-US", "en-t-posix-us"),
            ("en-t-abcde-Latn-rozaj-biske", "en-t-abcde-latn-biske-rozaj"),
            ("en-t-posix-DD", "en-t-posix-de"),
            ("en-t-posix-SU", "en-t-posix-ru"),
            ("en-t-abcde-us-m0-names", "en-t-abcde-us-m0-prprname"),
            ("posix-t-abcdef-fonipa", "posix-t-abcdef-fonipa"),
            ("en-u-ca-gregory-t-posix", "en-t-posix-u-ca-gregory"),
            ("en-x-t-abcdef", "en-x-t-abcdef"),
        ] {
            assert_eq!(canonicalize(tag).as_deref(), Some(want), "{tag:?}");
        }
        // UTS 35 Annex C: a tlang is canonicalized as a languageId, with
        // the language, script, region and variant aliases.
        for (tag, want) in [
            ("en-t-iw", "en-t-he"),
            ("en-t-ja-DD", "en-t-ja-de"),
            ("en-t-und-Qaai", "en-t-und-zinh"),
            // "SU" has several replacements; the likely region of "hy" is
            // one of them.
            ("en-t-hy-SU", "en-t-hy-am"),
            // Variant aliases: "heploc" is "alalc97" and "aaland" is the
            // region "AX".
            ("en-t-ja-latn-hepburn-heploc", "en-t-ja-latn-alalc97"),
            ("en-t-sv-aaland", "en-t-sv-ax"),
            // test262 intl402/Intl/getCanonicalLocales/
            // transformed-ext-canonical.js.
            ("sl-t-sl-rozaj-biske-1994", "sl-t-sl-1994-biske-rozaj"),
            ("DE-T-M0-DIN-K0-QWERTZ", "de-t-k0-qwertz-m0-din"),
            ("en-t-m0-true", "en-t-m0-true"),
            (
                "und-Latn-t-und-hani-m0-names",
                "und-Latn-t-und-hani-m0-prprname",
            ),
        ] {
            assert_eq!(canonicalize(tag).as_deref(), Some(want), "{tag:?}");
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
            "en-t-abcd",
            "en-t-abcdefghi",
        ] {
            assert_eq!(canonicalize(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn splits_off_the_language_id() {
        for (tag, id, rest) in [
            ("en", "en", ""),
            ("en-Latn-US-fonipa", "en-Latn-US-fonipa", ""),
            ("en-US-t-posix", "en-US", "-t-posix"),
            ("de-u-co-phonebk-x-a", "de", "-u-co-phonebk-x-a"),
            ("posix-x-private", "posix", "-x-private"),
            ("sl-1994-a-bc", "sl-1994", "-a-bc"),
        ] {
            assert_eq!(split_language_id(tag), (id, rest), "{tag:?}");
        }
        // The language identifier parses even where the whole tag does not.
        assert_eq!(
            language_id("en-US-t-posix")
                .map(|id| id.to_string())
                .as_deref(),
            Some("en-US")
        );
        assert_eq!(likely_script("sr-RS-t-posix"), Some(script!("Cyrl")));
        assert_eq!(language_id("posix"), None);
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
        assert_eq!(lookup("en-US-t-posix").as_deref(), Some("en-US"));
    }

    #[test]
    fn available_list_is_canonical() {
        assert!(available().len() > 400);
        for tag in available() {
            assert_eq!(canonicalize(tag).as_deref(), Some(tag.as_str()));
        }
    }
}

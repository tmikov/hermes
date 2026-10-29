// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! Turns the host's locale and time zone settings into ECMA-402 values. See
//! "Default locale and time zone" in the spec.

use crate::locale::resolve_default;
use icu::locale::subtags::Region;
use icu::time::zone::iana::IanaParserExtended;
use icu::time::zone::WindowsParser;
use icu::time::TimeZone;

/// A POSIX locale name (lang[_TERRITORY][.codeset][@modifier]) or an Apple
/// preferred-language string to a DefaultLocale candidate: a canonical tag
/// with no extensions that is in [[AvailableLocales]], by
/// locale::resolve_default once the POSIX syntax is turned into BCP 47.
/// None if it is not well-formed or no prefix is usable, so the caller tries
/// its next source.
pub fn locale_from_host(raw: &str) -> Option<String> {
    let (base, modifier) = match raw.split_once('@') {
        Some((base, modifier)) => (base, Some(modifier)),
        None => (raw, None),
    };
    let base = base.split('.').next().unwrap_or("");
    if base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX") {
        return Some("en-US".to_owned());
    }
    if base.is_empty() {
        return None;
    }
    let mut subtags: Vec<String> = base.split(['_', '-']).map(str::to_owned).collect();
    match modifier.map(str::to_ascii_lowercase).as_deref() {
        // The script modifiers glibc's locales use (/usr/share/i18n/SUPPORTED
        // lists latin, cyrillic, devanagari and iqtelif; its other modifiers
        // are @euro, @valencia and @abegede, a Ge'ez collation order).
        Some("latin") => insert_script(&mut subtags, "Latn"),
        Some("cyrillic") => insert_script(&mut subtags, "Cyrl"),
        Some("devanagari") => insert_script(&mut subtags, "Deva"),
        // Tatar in the Latin IQTElif alphabet (glibc tt_RU@iqtelif).
        Some("iqtelif") => insert_script(&mut subtags, "Latn"),
        Some("valencia") => subtags.push("valencia".to_owned()),
        // @euro and unknown modifiers carry no locale information.
        _ => {}
    }
    // Truncation must not drop the script subtag: tt-Latn-RU (glibc
    // tt_RU@iqtelif) would become "tt", which is Cyrillic.
    resolve_default(&subtags.join("-"))
}

/// Insert `script` after the language unless the tag already has a script.
fn insert_script(subtags: &mut Vec<String>, script: &str) {
    let has_script = subtags
        .get(1)
        .is_some_and(|s| s.len() == 4 && s.bytes().all(|b| b.is_ascii_alphabetic()));
    if !has_script {
        subtags.insert(1, script.to_owned());
    }
}

/// A `TZ` value or the target of the /etc/localtime link to a primary IANA
/// time zone name; None for POSIX rule strings, unknown zones, and paths
/// outside a zoneinfo directory. Below zoneinfo/, repeated slashes are
/// collapsed and glibc's posix/ and right/ (leap second) trees are skipped:
/// they hold the same zones under the same names.
pub fn time_zone_from_host(raw: &str) -> Option<String> {
    let name = raw.strip_prefix(':').unwrap_or(raw);
    let name = match name.rfind("zoneinfo/") {
        Some(i) => {
            let rest: Vec<&str> = name[i + "zoneinfo/".len()..]
                .split('/')
                .filter(|part| !part.is_empty())
                .collect();
            let rest = match rest.first() {
                Some(&"posix") | Some(&"right") => &rest[1..],
                _ => &rest[..],
            };
            rest.join("/")
        }
        None => name.to_owned(),
    };
    if name.is_empty() || name.starts_with('/') || name.starts_with('.') {
        return None;
    }
    time_zone_canonicalize(&name)
}

/// A case-insensitive IANA time zone name to its primary identifier; None if
/// the data does not know it.
pub fn time_zone_canonicalize(name: &str) -> Option<String> {
    let parsed = IanaParserExtended::new().parse(name);
    if parsed.time_zone == TimeZone::UNKNOWN {
        return None;
    }
    Some(primary_time_zone(parsed.canonical).to_owned())
}

/// ICU4X's canonical name for a zone to its ECMA-402 primary identifier
/// (AvailableNamedTimeZoneIdentifiers). ECMA-402 names UTC "UTC", although
/// IANA's canonical name is Etc/UTC. CLDR still keeps CST6CDT, EST5EDT,
/// MST7MDT and PST8PDT as zones of their own, although tzdb 2024b made them
/// links to the zones below (test262 intl402/Temporal/ZonedDateTime/links.js
/// checks all of tzdb's links). ICU4X already maps EST, MST and HST to their
/// tzdb targets.
pub fn primary_time_zone(canonical: &str) -> &str {
    match canonical {
        "Etc/UTC" | "Etc/GMT" | "GMT" => "UTC",
        "CST6CDT" => "America/Chicago",
        "EST5EDT" => "America/New_York",
        "MST7MDT" => "America/Denver",
        "PST8PDT" => "America/Los_Angeles",
        other => other,
    }
}

/// A Windows time zone key name (`TimeZoneKeyName` of
/// `GetDynamicTimeZoneInformation`, such as "Pacific Standard Time") to its
/// primary IANA identifier, by CLDR's windowsZones table. `region` is the
/// user's region (`GetUserDefaultGeoName`), which selects a territory's own
/// row ("Pacific Standard Time" in CA is America/Vancouver); with no region,
/// an unparsable one, or one that has no row for the zone, the territory
/// `001` row is used. None if the table does not know the key.
pub fn time_zone_from_windows(id: &str, region: Option<&str>) -> Option<String> {
    let parser = WindowsParser::new();
    let region = region.and_then(|r| Region::try_from_str(r).ok());
    // ICU4X looks up the exact (key, region) pair, so a region with no row
    // for this zone falls back to the 001 row here.
    let zone = parser
        .parse(id, region)
        .or_else(|| parser.parse(id, None))?;
    IanaParserExtended::new()
        .iter()
        .find(|entry| entry.time_zone == zone)
        .map(|entry| primary_time_zone(entry.canonical).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_locales() {
        let cases = [
            ("en_US.UTF-8", Some("en-US")),
            ("en_US.ISO-8859-1", Some("en-US")),
            ("sr_RS.UTF-8@latin", Some("sr-Latn-RS")),
            ("sr_RS@latin", Some("sr-Latn-RS")),
            ("sr_Latn_RS@latin", Some("sr-Latn-RS")),
            ("uz_UZ@cyrillic", Some("uz-Cyrl-UZ")),
            ("uz@cyrillic", Some("uz-Cyrl")),
            // The name's own script wins over the modifier's.
            ("sr_Cyrl_RS@latin", Some("sr-Cyrl-RS")),
            // A modifier alone has no language.
            ("@latin", None),
            ("ks_IN@devanagari", Some("ks-Deva-IN")),
            // tt-Latn-RU is not available, and its prefix "tt" is Cyrillic.
            ("tt_RU.UTF-8@iqtelif", None),
            // Available, but the data has only Adlam ff and Latin bal, and
            // these are likely Latin and Arabic.
            ("ff_SN.UTF-8", None),
            ("bal_PK", None),
            ("en_AQ.UTF-8", Some("en")),
            ("gez_ET@abegede", None),
            ("ca_ES.UTF-8@valencia", Some("ca-ES-valencia")),
            ("de_DE@euro", Some("de-DE")),
            ("C", Some("en-US")),
            ("C.UTF-8", Some("en-US")),
            ("POSIX", Some("en-US")),
            ("iw_IL", Some("he-IL")),
            ("zh-Hans-CN", Some("zh-Hans-CN")),
            ("zh_TW", Some("zh-TW")),
            ("posix", Some("en-US")),
            ("en_US-u-ca-buddhist", Some("en-US")),
            ("en-US-x-foo", Some("en-US")),
            ("english", None),
            ("German_Germany.1252", None),
            ("ber_MA", None),
            ("", None),
            ("garbage!!", None),
            (".UTF-8", None),
        ];
        for (raw, want) in cases {
            assert_eq!(locale_from_host(raw).as_deref(), want, "{raw:?}");
        }
    }

    #[test]
    fn host_time_zones() {
        let cases = [
            ("Europe/Paris", Some("Europe/Paris")),
            (":Europe/Paris", Some("Europe/Paris")),
            ("europe/paris", Some("Europe/Paris")),
            ("/usr/share/zoneinfo/Asia/Calcutta", Some("Asia/Kolkata")),
            (
                "../usr/share/zoneinfo/America/New_York",
                Some("America/New_York"),
            ),
            (
                "/var/db/timezone/zoneinfo/Europe/Berlin",
                Some("Europe/Berlin"),
            ),
            (
                "/usr/share/zoneinfo/posix/Europe/Paris",
                Some("Europe/Paris"),
            ),
            (
                "/usr/share/zoneinfo/right/America/New_York",
                Some("America/New_York"),
            ),
            ("/usr/share/zoneinfo//Europe/Paris", Some("Europe/Paris")),
            (
                "/usr/share/zoneinfo/posix//Asia/Calcutta",
                Some("Asia/Kolkata"),
            ),
            ("/usr/share/zoneinfo/posix/", None),
            ("/usr/share/zoneinfo/posixrules", None),
            ("UTC", Some("UTC")),
            ("Etc/UTC", Some("UTC")),
            ("GMT", Some("UTC")),
            ("GMT0", Some("UTC")),
            ("Etc/GMT", Some("UTC")),
            // POSIX rule strings, although they mean UTC and UTC+3.
            ("UTC0", None),
            ("<+03>-3", None),
            // Links since tzdb 2024b, although CLDR keeps them as zones.
            ("CST6CDT", Some("America/Chicago")),
            ("EST5EDT", Some("America/New_York")),
            ("mst7mdt", Some("America/Denver")),
            ("/usr/share/zoneinfo/PST8PDT", Some("America/Los_Angeles")),
            // Already a link in ICU4X's data.
            ("EST", Some("America/Panama")),
            ("EST5EDT,M3.2.0,M11.1.0", None),
            ("", None),
            ("/etc/foo", None),
            ("Mars/Olympus_Mons", None),
        ];
        for (raw, want) in cases {
            assert_eq!(time_zone_from_host(raw).as_deref(), want, "{raw:?}");
        }
    }

    #[test]
    fn windows_time_zones() {
        let cases = [
            ("Pacific Standard Time", None, Some("America/Los_Angeles")),
            ("India Standard Time", None, Some("Asia/Kolkata")),
            ("UTC", None, Some("UTC")),
            // A territory's own row.
            (
                "Pacific Standard Time",
                Some("CA"),
                Some("America/Vancouver"),
            ),
            (
                "Pacific Standard Time",
                Some("ca"),
                Some("America/Vancouver"),
            ),
            (
                "Eastern Standard Time",
                Some("US"),
                Some("America/New_York"),
            ),
            // The region has no row for this zone: the 001 row.
            ("India Standard Time", Some("US"), Some("Asia/Kolkata")),
            ("W. Europe Standard Time", Some("JP"), Some("Europe/Berlin")),
            // Not a region at all: the 001 row.
            (
                "Pacific Standard Time",
                Some("not a region"),
                Some("America/Los_Angeles"),
            ),
            (
                "Pacific Standard Time",
                Some(""),
                Some("America/Los_Angeles"),
            ),
            // A UN M.49 code, which GetUserDefaultGeoName can return.
            (
                "Pacific Standard Time",
                Some("419"),
                Some("America/Los_Angeles"),
            ),
            // Keys are case-sensitive in the table, as in the registry's
            // names.
            ("pacific standard time", None, None),
            ("Mars Standard Time", None, None),
            ("", None, None),
            ("Pacific Standard Time/CA", None, None),
        ];
        for (id, region, want) in cases {
            assert_eq!(
                time_zone_from_windows(id, region).as_deref(),
                want,
                "{id:?} {region:?}"
            );
        }
    }

    /// Every key of CLDR 48.2's windowsZones that has a territory 001 row
    /// maps to a primary IANA identifier (Review Focus 5).
    #[test]
    fn every_windows_zone_maps_to_a_primary_identifier() {
        use icu::time::provider::windows::TimezoneIdentifiersWindowsV1;
        use icu::time::provider::Baked;
        use icu_provider::prelude::*;
        let response: DataResponse<TimezoneIdentifiersWindowsV1> =
            DataProvider::<TimezoneIdentifiersWindowsV1>::load(&Baked, Default::default())
                .expect("windowsZones data");
        let keys: Vec<String> = response
            .payload
            .get()
            .map
            .iter()
            .filter_map(|(key, _)| key.strip_suffix("/001").map(str::to_owned))
            .collect();
        // cldr-core 48.2.0's windowsZones.json has 139 territory 001 rows.
        assert_eq!(keys.len(), 139);
        for key in &keys {
            let zone = time_zone_from_windows(key, None)
                .unwrap_or_else(|| panic!("{key:?} maps to nothing"));
            assert_eq!(
                time_zone_canonicalize(&zone).as_deref(),
                Some(zone.as_str()),
                "{key:?} maps to {zone:?}, which is not a primary identifier"
            );
        }
    }
}

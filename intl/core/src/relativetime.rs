// Copyright (c) Meta Platforms, Inc. and affiliates.
//
// This source code is licensed under the MIT license found in the
// LICENSE file in the root directory of this source tree.

//! The CLDR data behind Intl.RelativeTimeFormat (plan decision 4 of Phase
//! 5): icu_experimental's 24 relative time markers, one per style and unit.
//! JS performs every operation of the draft; native code supplies only the
//! patterns and the relative names of a locale and style.

use crate::locale::language_id;
use crate::number::{by_category, load, plural_categories, render_compact};
use icu_experimental::provider::Baked as ExperimentalBaked;
use icu_experimental::relativetime::provider::*;
use icu_provider::prelude::*;
use zerovec::ule::AsULE;

/// RelativeTimeFormat's data for the language identifier of `locale` (a
/// canonical tag) and `style` "long", "short" or "narrow". For each unit
/// in the order second, minute, hour, day, week, month, quarter, year:
/// entries 0-5 the future patterns and 6-11 the past patterns for zero,
/// one, two, few, many and other, in CLDR's form (`in {0} days`; a pattern
/// may lack `{0}`; a category the locale lacks gets other's), then the
/// number `N` of relative names, then `N` pairs: the offset as a decimal
/// string (`-1`) and the name (`yesterday`), in increasing offset. None
/// for any other style, if `locale` does not parse, or if there is no
/// data.
pub fn patterns(locale: &str, style: &str) -> Option<Vec<String>> {
    let id = language_id(locale)?;
    let locale_data = DataLocale::from(&id);
    let categories = plural_categories(locale)?;
    let units = payloads(&locale_data, style)?;
    let mut out = Vec::new();
    for payload in &units {
        let data = payload.get();
        for tense in [&data.future, &data.past] {
            out.extend(
                by_category(&tense.elements, &categories)
                    .into_iter()
                    .map(render_compact),
            );
        }
        out.push(data.relatives.len().to_string());
        for (offset, name) in data.relatives.iter() {
            out.push(i8::from_unaligned(*offset).to_string());
            out.push(name.to_owned());
        }
    }
    Some(out)
}

/// The data of the 8 relative time markers of `style` ("long", "short" or
/// "narrow") for `locale`, in the order second, minute, hour, day, week,
/// month, quarter, year, each as the payload of one marker type: all 24
/// markers have the same data struct. This is the one list of the
/// markers. None for any other style, or if there is no data.
fn payloads(locale: &DataLocale, style: &str) -> Option<[DataPayload<LongSecondRelativeV1>; 8]> {
    Some(match style {
        "long" => [
            unit::<LongSecondRelativeV1>(locale)?,
            unit::<LongMinuteRelativeV1>(locale)?,
            unit::<LongHourRelativeV1>(locale)?,
            unit::<LongDayRelativeV1>(locale)?,
            unit::<LongWeekRelativeV1>(locale)?,
            unit::<LongMonthRelativeV1>(locale)?,
            unit::<LongQuarterRelativeV1>(locale)?,
            unit::<LongYearRelativeV1>(locale)?,
        ],
        "short" => [
            unit::<ShortSecondRelativeV1>(locale)?,
            unit::<ShortMinuteRelativeV1>(locale)?,
            unit::<ShortHourRelativeV1>(locale)?,
            unit::<ShortDayRelativeV1>(locale)?,
            unit::<ShortWeekRelativeV1>(locale)?,
            unit::<ShortMonthRelativeV1>(locale)?,
            unit::<ShortQuarterRelativeV1>(locale)?,
            unit::<ShortYearRelativeV1>(locale)?,
        ],
        "narrow" => [
            unit::<NarrowSecondRelativeV1>(locale)?,
            unit::<NarrowMinuteRelativeV1>(locale)?,
            unit::<NarrowHourRelativeV1>(locale)?,
            unit::<NarrowDayRelativeV1>(locale)?,
            unit::<NarrowWeekRelativeV1>(locale)?,
            unit::<NarrowMonthRelativeV1>(locale)?,
            unit::<NarrowQuarterRelativeV1>(locale)?,
            unit::<NarrowYearRelativeV1>(locale)?,
        ],
        _ => return None,
    })
}

/// The data of the relative time marker `M` for `locale`, as the payload
/// of one marker type. None if there is no data.
fn unit<M>(locale: &DataLocale) -> Option<DataPayload<LongSecondRelativeV1>>
where
    M: DataMarker<DataStruct = RelativeTimePatternData<'static>>,
    ExperimentalBaked: DataProvider<M>,
{
    Some(load::<M>(&ExperimentalBaked, locale, "")?.cast())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The units of `patterns`, in its order.
    const UNITS: [&str; 8] = [
        "second", "minute", "hour", "day", "week", "month", "quarter", "year",
    ];

    /// One unit of `patterns`: the 6 future patterns, the 6 past ones, and
    /// the relative names as (offset, name) pairs.
    struct Unit {
        future: Vec<String>,
        past: Vec<String>,
        names: Vec<(String, String)>,
    }

    /// `patterns(locale, style)`, which must exist, split into its units,
    /// keyed by name.
    fn units(locale: &str, style: &str) -> Vec<(&'static str, Unit)> {
        let list = patterns(locale, style).unwrap_or_else(|| panic!("{locale} {style}"));
        let mut i = 0;
        let mut out = Vec::new();
        for unit in UNITS {
            let future = list[i..i + 6].to_vec();
            let past = list[i + 6..i + 12].to_vec();
            let n: usize = list[i + 12].parse().unwrap();
            i += 13;
            let names = (0..n)
                .map(|k| (list[i + 2 * k].clone(), list[i + 2 * k + 1].clone()))
                .collect();
            i += 2 * n;
            out.push((
                unit,
                Unit {
                    future,
                    past,
                    names,
                },
            ));
        }
        assert_eq!(i, list.len(), "{locale} {style}");
        out
    }

    /// The unit `unit` of `patterns(locale, style)`.
    fn unit(locale: &str, style: &str, unit: &str) -> Unit {
        units(locale, style)
            .into_iter()
            .find(|(u, _)| *u == unit)
            .unwrap()
            .1
    }

    /// The relative name of `offset` in `u`, if it has one.
    fn name<'a>(u: &'a Unit, offset: &str) -> Option<&'a str> {
        u.names
            .iter()
            .find(|(o, _)| o == offset)
            .map(|(_, n)| n.as_str())
    }

    /// The indices of the plural categories in the six patterns of a tense.
    const ONE: usize = 1;
    const FEW: usize = 3;
    const MANY: usize = 4;
    const OTHER: usize = 5;

    #[test]
    fn english() {
        let day = unit("en", "long", "day");
        assert_eq!(day.future[OTHER], "in {0} days");
        assert_eq!(day.future[ONE], "in {0} day");
        assert_eq!(day.past[OTHER], "{0} days ago");
        assert_eq!(name(&day, "-1"), Some("yesterday"));
        assert_eq!(name(&day, "0"), Some("today"));
        assert_eq!(name(&day, "1"), Some("tomorrow"));
        assert_eq!(unit("en", "short", "week").past[OTHER], "{0} wk. ago");
        assert_eq!(unit("en", "narrow", "month").future[OTHER], "in {0}mo");
    }

    #[test]
    fn plurals() {
        let year = unit("pl", "long", "year");
        assert_eq!(year.future[FEW], "za {0} lata");
        assert_eq!(year.future[MANY], "za {0} lat");
        // A pattern without {0}.
        assert_eq!(unit("ar", "long", "second").past[ONE], "قبل ثانية واحدة");
    }

    #[test]
    fn names() {
        assert_eq!(name(&unit("de", "long", "day"), "-2"), Some("vorgestern"));
        assert_eq!(name(&unit("gd", "long", "day"), "3"), Some("an-eararais"));
        assert_eq!(name(&unit("ja", "long", "day"), "2"), Some("明後日"));
    }

    /// Plan Review Focus 5: every locale and style has 8 units of 12
    /// patterns, and no locale has an explicit =0 or =1 pattern, which
    /// `patterns` would not return; a CLDR change that adds one fails here.
    #[test]
    fn every_locale() {
        use crate::number::explicit;

        for locale in crate::locale::test_locales() {
            for style in ["long", "short", "narrow"] {
                let units = units(locale, style);
                assert_eq!(units.len(), 8, "{locale} {style}");
                for (u, data) in &units {
                    assert_eq!(data.future.len(), 6, "{locale} {style} {u}");
                    assert_eq!(data.past.len(), 6, "{locale} {style} {u}");
                    assert!(
                        data.future.iter().chain(&data.past).all(|p| !p.is_empty()),
                        "{locale} {style} {u}"
                    );
                }
            }
            let id = crate::locale::language_id(locale).unwrap();
            let l = DataLocale::from(&id);
            for style in ["long", "short", "narrow"] {
                let units = payloads(&l, style).unwrap_or_else(|| panic!("{locale} {style}"));
                for (payload, u) in units.iter().zip(UNITS) {
                    let data = payload.get();
                    for tense in [&data.future.elements, &data.past.elements] {
                        for one in [false, true] {
                            assert_eq!(
                                explicit(tense, one, render_compact),
                                "",
                                "{locale} {style} {u} {one}"
                            );
                        }
                    }
                }
            }
        }
    }

    /// Each style and unit reads its own marker: fr's 24 future "other"
    /// patterns all differ (short has U+00A0 before s, min, h and j).
    #[test]
    fn every_marker() {
        let expected = [
            (
                "long",
                [
                    "dans {0} secondes",
                    "dans {0} minutes",
                    "dans {0} heures",
                    "dans {0} jours",
                    "dans {0} semaines",
                    "dans {0} mois",
                    "dans {0} trimestres",
                    "dans {0} ans",
                ],
            ),
            (
                "short",
                [
                    "dans {0}\u{a0}s",
                    "dans {0}\u{a0}min",
                    "dans {0}\u{a0}h",
                    "dans {0}\u{a0}j",
                    "dans {0} sem.",
                    "dans {0} m.",
                    "dans {0} trim.",
                    "dans {0} a",
                ],
            ),
            (
                "narrow",
                [
                    "+{0} s",
                    "+{0} min",
                    "+{0} h",
                    "+{0} j",
                    "+{0} sem.",
                    "+{0} m.",
                    "+{0} trim.",
                    "+{0} a",
                ],
            ),
        ];
        for (style, patterns) in expected {
            let got: Vec<String> = units("fr", style)
                .into_iter()
                .map(|(_, u)| u.future[OTHER].clone())
                .collect();
            assert_eq!(got, patterns, "{style}");
        }
    }

    /// The relative names come in increasing offset.
    #[test]
    fn name_order() {
        let pairs = |u: Unit| u.names;
        let s = |a: &str, b: &str| (a.to_owned(), b.to_owned());
        assert_eq!(
            pairs(unit("en", "long", "day")),
            [s("-1", "yesterday"), s("0", "today"), s("1", "tomorrow")]
        );
        assert_eq!(
            pairs(unit("de", "long", "day")),
            [
                s("-2", "vorgestern"),
                s("-1", "gestern"),
                s("0", "heute"),
                s("1", "morgen"),
                s("2", "übermorgen"),
            ]
        );
    }

    #[test]
    fn rejects() {
        assert!(patterns("en", "wide").is_none());
        assert!(patterns("en_US", "long").is_none());
    }
}

#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Unit tests of update-cldr-lists.py's extraction of the -u- and -t- value
aliases (on fixture bcp47 XML), of number-symbols.txt, units.txt and
currency-symbols.txt (on fixture CLDR JSON), of explicit-number-systems.txt
(on fixture common/main XML), of compact-currency.txt and date-patterns.txt
(on both) and of day-periods.txt and display-names.txt (on fixture CLDR
JSON). Run by check-intl-data; no network."""

import importlib.util
import json
import os
import tempfile
import unittest
import xml.etree.ElementTree as ET

_spec = importlib.util.spec_from_file_location(
    "update_cldr_lists",
    os.path.join(os.path.dirname(os.path.abspath(__file__)), "update-cldr-lists.py"),
)
gen = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(gen)

# Shaped like CLDR's bcp47/*.xml. Each type exercises one rule.
FIXTURE_U = """<ldmlBCP47><keyword>
  <key name="tz" alias="timezone">
    <type name="aaone" alias="Area/One Oldname"/>
    <type name="bbtwo" alias="Area/Two"/>
    <type name="ccold" deprecated="true" preferred="bbtwo"/>
    <type name="ddold" deprecated="true" preferred="ccold" alias="DDOLD Ddalias"/>
    <type name="eegone" deprecated="true"/>
    <type name="ffname" alias="Area/F Aaone"/>
    <type name="ggkept" preferred="bbtwo"/>
  </key>
  <key name="ca" alias="calendar">
    <type name="longname" alias="toolongname"/>
    <type name="islamic-civil"/>
    <type name="islamicc" deprecated="true" preferred="islamic-civil"
          alias="islamic-civil"/>
    <type name="ethioaa" alias="ethiopic-amete-alem"/>
    <type name="gb2312" deprecated="true" alias="gb2312x"/>
  </key>
  <key name="kn" alias="colNumeric">
    <type name="true" alias="yes"/>
    <type name="false" alias="no"/>
  </key>
</keyword></ldmlBCP47>"""

FIXTURE_T = """<ldmlBCP47><keyword>
  <key extension="t" name="m0">
    <type name="prprname" alias="names"/>
    <type name="c11" alias="c"/>
    <type name="iesjes" alias="ies-jes"/>
  </key>
  <key extension="t" name="kn">
    <type name="tvalue" alias="yes"/>
  </key>
</keyword></ldmlBCP47>"""


def extract(*xml):
    return gen.extension_value_aliases([ET.fromstring(x) for x in xml])


class ExtensionValueAliasesTest(unittest.TestCase):
    def setUp(self):
        self.table, skipped = extract(FIXTURE_U, FIXTURE_T)
        self.skipped = {(e, k, a): why for e, k, a, why in skipped}

    def test_deprecated_types_map_to_their_preferred_value(self):
        self.assertEqual(self.table[("u", "tz", "ccold")], "bbtwo")
        self.assertEqual(self.table[("u", "ca", "islamicc")], "islamic-civil")

    def test_preferred_chains_are_followed(self):
        self.assertEqual(self.table[("u", "tz", "ddold")], "bbtwo")

    def test_aliases_map_to_the_type_name_lowercased(self):
        self.assertEqual(self.table[("u", "tz", "oldname")], "aaone")
        self.assertEqual(self.table[("u", "ca", "ethiopic-amete-alem")], "ethioaa")
        self.assertEqual(self.table[("t", "m0", "names")], "prprname")
        self.assertEqual(self.table[("t", "m0", "ies-jes")], "iesjes")

    def test_an_alias_of_a_deprecated_type_maps_to_its_preferred_value(self):
        self.assertEqual(self.table[("u", "tz", "ddalias")], "bbtwo")

    def test_deprecated_types_without_preferred_stay(self):
        self.assertNotIn(("u", "tz", "eegone"), self.table)
        # Its aliases still map to it.
        self.assertEqual(self.table[("u", "ca", "gb2312x")], "gb2312")

    def test_only_deprecated_types_map_to_their_preferred_value(self):
        self.assertNotIn(("u", "tz", "ggkept"), self.table)

    def test_ill_formed_aliases_are_skipped(self):
        for ext, key, alias in [
            ("u", "tz", "Area/One"),
            ("u", "ca", "toolongname"),
            ("u", "kn", "no"),
            ("t", "m0", "c"),
        ]:
            self.assertEqual(
                self.skipped[(ext, key, alias)], "not a well-formed value", alias
            )
            self.assertNotIn((ext, key, alias.lower()), self.table)

    def test_type_names_stay(self):
        self.assertNotIn(("u", "ca", "islamic-civil"), self.table)
        self.assertNotIn(("u", "tz", "aaone"), self.table)
        self.assertEqual(
            self.skipped[("u", "ca", "islamic-civil")], "the name of a type"
        )
        self.assertEqual(self.skipped[("u", "tz", "Aaone")], "the name of a type")
        # A type's alias that is its own name is not recorded at all.
        self.assertNotIn(("u", "tz", "DDOLD"), self.skipped)

    def test_extensions_are_kept_apart(self):
        self.assertEqual(self.table[("u", "kn", "yes")], "true")
        self.assertEqual(self.table[("t", "kn", "yes")], "tvalue")

    def test_nothing_else(self):
        self.assertEqual(
            sorted(self.table),
            sorted(
                [
                    ("u", "tz", "ccold"),
                    ("u", "tz", "ddold"),
                    ("u", "tz", "oldname"),
                    ("u", "tz", "ddalias"),
                    ("u", "ca", "islamicc"),
                    ("u", "ca", "ethiopic-amete-alem"),
                    ("u", "ca", "gb2312x"),
                    ("u", "kn", "yes"),
                    ("t", "m0", "names"),
                    ("t", "m0", "ies-jes"),
                    ("t", "kn", "yes"),
                ]
            ),
        )

    def test_conflicting_aliases_are_an_error(self):
        conflict = """<ldmlBCP47><keyword><key name="tz">
          <type name="aaone" alias="Same"/>
          <type name="bbtwo" alias="Same"/>
        </key></keyword></ldmlBCP47>"""
        with self.assertRaises(SystemExit):
            extract(conflict)

    def test_a_preferred_loop_is_an_error(self):
        loop = """<ldmlBCP47><keyword><key name="tz">
          <type name="aaone" deprecated="true" preferred="bbtwo"/>
          <type name="bbtwo" deprecated="true" preferred="aaone"/>
        </key></keyword></ldmlBCP47>"""
        with self.assertRaises(SystemExit):
            extract(loop)


def symbols(decimal, percent="%", infinity="∞"):
    return {
        "decimal": decimal,
        "group": ",",
        "percentSign": percent,
        "plusSign": "+",
        "minusSign": "-",
        "approximatelySign": "~",
        "exponential": "E",
        "infinity": infinity,
        "nan": "NaN",
        "timeSeparator": ":",
    }


def numbers(*systems):
    """The `numbers` object of a numbers.json with the (nu, decimal, percent
    pattern) systems; the first is the default one."""
    n = {"defaultNumberingSystem": systems[0][0]}
    for nu, decimal, pattern in systems:
        n[f"symbols-numberSystem-{nu}"] = symbols(decimal)
        n[f"percentFormats-numberSystem-{nu}"] = {"standard": pattern}
        n[f"miscPatterns-numberSystem-{nu}"] = {"range": "{0}–{1}"}
    return n


def unit(one, other, per=None):
    u = {"unitPattern-count-one": one, "unitPattern-count-other": other}
    if per is not None:
        u["perUnitPattern"] = per
    return u


def units(**by_width):
    """The `units` object of a units.json from {width: {key: value}}."""
    return {"long": {}, "short": {}, "narrow": {}, **by_width}


KM = unit("{0} km", "{0} km", "{0}/km")
M = unit("{0} m", "{0} m", "{0}/m")
PER = {"compoundUnitPattern": "{0}/{1}"}

# Locale und is CLDR's root; en-AU's parent entry is en-001, which the test
# file does not have, so its parent in the file is en.
FIXTURE_NUMBERS = {
    "und": numbers(("latn", ".", "#,##0%")),
    "en": numbers(("latn", ".", "#,##0%")),
    "en-AU": numbers(("latn", ".", "#,##0%")),
    "ar": numbers(("arab", "٫", "#,##0‏%"), ("latn", ".", "#,##0%")),
}
FIXTURE_UNITS = {
    "und": units(
        short={
            "length-kilometer": KM,
            "length-meter": M,
            "speed-kilometer-per-hour": unit("{0} km/h", "{0} km/h"),
            "speed-foo-per-hour": unit("{0} f/h", "{0} f/h"),
            "consumption-mile-per-gallon-imperial": unit("{0} mpg", "{0} mpg"),
            "per": PER,
        },
        long={"length-meter": unit("{0} meter", "{0} meters"), "per": PER},
    ),
    "en": units(
        short={
            "length-kilometer": KM,
            "length-meter": M,
            "speed-kilometer-per-hour": unit("{0} km/h", "{0} km/h"),
            "speed-foo-per-hour": unit("{0} f/h", "{0} f/h"),
            "consumption-mile-per-gallon-imperial": unit("{0} mpg", "{0} mpg"),
            "per": PER,
        },
        long={"length-meter": unit("{0} meter", "{0} meters"), "per": PER},
    ),
    "en-AU": units(
        short={
            "length-kilometer": KM,
            "length-meter": unit("{0} metre", "{0} metres", "{0}/m"),
            "speed-kilometer-per-hour": unit("{0} km/h", "{0} km/h"),
            "speed-foo-per-hour": unit("{0} f/h", "{0} f/h"),
            "consumption-mile-per-gallon-imperial": unit("{0} mpg", "{0} mpg"),
            "per": PER,
        },
        long={"length-meter": unit("{0} meter", "{0} meters"), "per": PER},
    ),
    "ar": units(
        short={
            "length-kilometer": KM,
            "length-meter": M,
            "speed-kilometer-per-hour": unit("{0} km/h", "{0} km/h"),
            "per": PER,
        },
        long={"length-meter": unit("{0} meter", "{0} meters"), "per": PER},
    ),
}
FIXTURE_CURRENCIES = {
    "und": {
        "USD": {"symbol": "US$", "symbol-alt-narrow": "$"},
        "XXX": {"symbol": "¤"},
        "BBB": {},
    },
    "en": {
        "USD": {"symbol": "$", "symbol-alt-narrow": "$"},
        "XXX": {"symbol": "¤"},
        "BBB": {},
        "PTE": {"symbol": "Esc"},
    },
    "en-AU": {
        "USD": {"symbol": "USD", "symbol-alt-narrow": "USD"},
        "AUD": {"symbol": "$", "displayName": "Australian Dollar"},
        "XXX": {"symbol": "¤"},
        "PTE": {"symbol": "Esc"},
    },
    "ar": {
        "USD": {"symbol": "US$", "symbol-alt-narrow": "$"},
        "XXX": {"symbol": "¤"},
    },
}
FIXTURE_PARENTS = {"en-AU": "en-001"}
FIXTURE_LIKELY = {
    "zh-HK": "zh-Hant-HK",
    "zh": "zh-Hans-CN",
    "zz": "zz-Latn-ZZ",
}
SANCTIONED = {"kilometer", "meter", "hour", "mile", "gallon"}


def write_json(root, parts, value):
    path = os.path.join(root, *parts)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(value, f)


def make_tree(
    root, number_data, unit_data, parents=None, likely=None, currency_data=None
):
    """Write fixture CLDR JSON packages under `root`, in the layout of the
    unpacked npm tarballs, and return a tree reading them."""
    for loc, n in number_data.items():
        write_json(
            root,
            ["cldr-numbers-full", "package", "main", loc, "numbers.json"],
            {"main": {loc: {"numbers": n}}},
        )
    for loc, c in (currency_data or {}).items():
        write_json(
            root,
            ["cldr-numbers-full", "package", "main", loc, "currencies.json"],
            {"main": {loc: {"numbers": {"currencies": c}}}},
        )
    for loc, u in unit_data.items():
        write_json(
            root,
            ["cldr-units-full", "package", "main", loc, "units.json"],
            {"main": {loc: {"units": u}}},
        )
    write_json(
        root,
        ["cldr-core", "package", "supplemental", "parentLocales.json"],
        {"supplemental": {"parentLocales": {"parentLocale": parents or {}}}},
    )
    write_json(
        root,
        ["cldr-core", "package", "supplemental", "likelySubtags.json"],
        {"supplemental": {"likelySubtags": likely or {}}},
    )
    return gen.Tree(root)


def parse_units(lines):
    """Parse units.txt lines: (strings, {tag: (parent, {(width, unit): {key:
    pattern}})}) with the indices resolved."""
    lines = [l for l in lines if not l.startswith("#")]
    head, count = lines[0].split()
    assert head == "strings"
    strings = lines[1 : 1 + int(count)]
    out = {}
    cur = None
    for line in lines[1 + int(count) :]:
        f = line.split(" ")
        if f[0] == "locale":
            cur = out[f[1]] = (f[2], {})
        else:
            cur[1][(f[0], f[1])] = {
                k: strings[int(i)] for k, i in (kv.split("=") for kv in f[2:])
            }
    return strings, out


# common/main XML for the per-currency formats of currency_symbols_lines.
CURRENCY_EN_XML = """<ldml><numbers><currencies>
<currency type="PTE"><symbol>Esc</symbol><pattern>#,##0.00\u00a0¤</pattern>
<decimal>$</decimal><group>↑↑↑</group></currency>
<currency type="USD"><displayName>US Dollar</displayName></currency>
</currencies></numbers></ldml>"""
CURRENCY_EN_AU_XML = """<ldml><numbers>
<symbols numberSystem="latn"><decimal>.</decimal><group draft="unconfirmed">x</group></symbols>
<currencyFormats numberSystem="latn"><currencyFormatLength>
<currencyFormat type="standard"><pattern alt="alphaNextToNumber">¤ #,##0.00</pattern></currencyFormat>
<currencyFormat type="accounting"><pattern>¤#,##0.00;(¤#,##0.00)</pattern></currencyFormat>
</currencyFormatLength></currencyFormats>
<currencies><currency type="PTE"><group>,</group></currency></currencies>
</numbers></ldml>"""


class CldrNumberDataTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.root = self._tmp.name

    def tree(
        self,
        number_data=None,
        unit_data=None,
        parents=None,
        likely=None,
        currency_data=None,
    ):
        return make_tree(
            self.root,
            FIXTURE_NUMBERS if number_data is None else number_data,
            FIXTURE_UNITS if unit_data is None else unit_data,
            FIXTURE_PARENTS if parents is None else parents,
            FIXTURE_LIKELY if likely is None else likely,
            FIXTURE_CURRENCIES if currency_data is None else currency_data,
        )

    def currency_symbols(self, currency_data=None, xml=None):
        """Parse currency_symbols_lines: {tag: (parent, {code: (symbol,
        narrow[, pattern, decimal, group])})}, with the lateral lines under
        "lateral" as a list."""
        tree = self.tree(currency_data=currency_data)
        files = {"en": CURRENCY_EN_XML, "en_AU": CURRENCY_EN_AU_XML}
        files.update(xml or {})
        lines = gen.currency_symbols_lines(
            tree,
            ["en", "en-AU", "ar"],
            lambda name: ET.fromstring(files.get(name, "<ldml/>")),
        )
        out = {}
        for line in lines:
            f = line.split("\t")
            if f[0] == "locale":
                self.assertEqual(len(f), 3, line)
                cur = out[f[1]] = (f[2], {})
            elif f[0] == "lateral":
                self.assertEqual(len(f), 4, line)
                cur[1].setdefault("lateral", []).append(tuple(f[1:]))
            else:
                self.assertIn(len(f), (3, 6), line)
                cur[1][f[0]] = tuple(f[1:])
        return out

    def test_currency_symbols_dedup_against_parent(self):
        locales = self.currency_symbols()
        # und lists what differs from the code: XXX's narrow symbol is its
        # symbol, and BBB has neither, so it uses its code.
        self.assertEqual(
            locales["und"],
            ("-", {"USD": ("US$", "$"), "XXX": ("¤", "¤")}),
        )
        # A symbol equal to the code is kept when the parent's differs.
        # A locale's own per-currency format goes after the symbols (en-AU
        # adds only PTE's group), and the values per-currency fields inherit
        # laterally from follow, for a locale whose parents have formats.
        self.assertEqual(
            locales["en"],
            (
                "und",
                {
                    "USD": ("$", ""),
                    "PTE": ("Esc", "Esc", "#,##0.00\u00a0¤", "$", ""),
                },
            ),
        )
        self.assertEqual(
            locales["en-AU"],
            (
                "en",
                {
                    "USD": ("USD", "USD"),
                    "AUD": ("$", "$"),
                    "PTE": ("", "", "", "", ","),
                    "lateral": [
                        ("decimal", "latn", "-"),
                        ("pattern", "latn", "accounting"),
                    ],
                },
            ),
        )
        self.assertEqual(locales["ar"], ("und", {}))

    def test_currency_symbols_must_reproduce_the_locale(self):
        c = dict(FIXTURE_CURRENCIES)
        # en-AU leaves out USD, which en's chain gives "$" for.
        c["en-AU"] = {
            "AUD": {"symbol": "$"},
            "XXX": {"symbol": "¤"},
            "PTE": FIXTURE_CURRENCIES["en-AU"]["PTE"],
        }
        with self.assertRaisesRegex(SystemExit, "en-AU USD"):
            self.currency_symbols(c)
        c["en-AU"] = {"USD": {"symbol": "U\tS"}}
        with self.assertRaisesRegex(SystemExit, "tab"):
            self.currency_symbols(c)
        c["en-AU"] = {"usd": {"symbol": "$"}}
        with self.assertRaisesRegex(SystemExit, "bad currency code"):
            self.currency_symbols(c)
        # A pattern number.rs does not handle.
        bad = CURRENCY_EN_XML.replace("#,##0.00\u00a0¤", "¤#,##,##0.00")
        with self.assertRaisesRegex(SystemExit, "unhandled per-currency pattern"):
            self.currency_symbols(xml={"en": bad})
        # A locale of a chain that the file does not hold (en-001, en-AU's
        # CLDR parent) with per-currency formats.
        with self.assertRaisesRegex(SystemExit, "en-001 has per-currency"):
            self.currency_symbols(xml={"en_001": CURRENCY_EN_XML})

    def test_per_currency_lateral_inheritance(self):
        # en has PTE's pattern and decimal; en-AU has PTE's group and its
        # own latn decimal and accounting pattern, which come before en's
        # for latn (lateral inheritance). The generator reads the file back
        # against the XML, so these lines must be there: without them en-AU
        # latn would get en's decimal "$". An unconfirmed value and an
        # alternate-only pattern are not lateral sources.
        locales = self.currency_symbols()
        self.assertEqual(
            locales["en-AU"][1]["lateral"],
            [("decimal", "latn", "-"), ("pattern", "latn", "accounting")],
        )
        self.assertNotIn("lateral", locales["en"][1])

    def test_currency_spacing_must_be_roots(self):
        spacing = gen.CURRENCY_SPACING
        n = {t: dict(v) for t, v in FIXTURE_NUMBERS.items()}
        for t in n:
            n[t]["currencyFormats-numberSystem-latn"] = {"currencySpacing": spacing}
        gen.check_currency_spacing(self.tree(n), ["en", "ar"])
        side = dict(gen.CURRENCY_SPACING_SIDE, insertBetween=" ")
        n["ar"]["currencyFormats-numberSystem-arab"] = {
            "currencySpacing": {"beforeCurrency": side, "afterCurrency": side}
        }
        with self.assertRaisesRegex(SystemExit, "ar currencyFormats"):
            gen.check_currency_spacing(self.tree(n), ["en", "ar"])
        # A root rule other than the one number.rs implements fails too.
        n["ar"] = dict(n["en"])
        n["und"] = dict(n["und"])
        n["und"]["currencyFormats-numberSystem-latn"] = {}
        with self.assertRaisesRegex(SystemExit, "und currencyFormats"):
            gen.check_currency_spacing(self.tree(n), ["en"])

    def units(self, tags=("en", "en-AU", "ar"), **kw):
        tree = self.tree(**kw)
        return parse_units(gen.units_lines(tree, list(tags), SANCTIONED))

    def test_number_symbols_lines(self):
        tree = self.tree()
        lines = gen.number_symbols_lines(tree, ["en", "ar"])
        self.assertEqual(
            [l for l in lines if not l.startswith("#")],
            [
                "ar\tarab\t%\tE\t∞\tNaN\t~\t#,##0‏%\t{0}–{1}\t\t\t:",
                "ar\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t\t:",
                "en\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t\t:",
                "und\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t\t:",
            ],
        )

    def test_currency_separators(self):
        n = dict(FIXTURE_NUMBERS)
        n["en"] = numbers(("latn", ".", "#,##0%"))
        n["en"]["symbols-numberSystem-latn"]["currencyGroup"] = "."
        n["en"]["symbols-numberSystem-latn"]["currencyDecimal"] = "\u00b7"
        lines = gen.number_symbols_lines(self.tree(n), ["en"])
        self.assertIn(
            "en\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\u00b7\t.\t:", lines
        )

    def test_units_dedup_against_parent(self):
        _, locales = self.units()
        # en-AU's km equals en's (so nothing is listed); its short meter differs.
        self.assertEqual(
            locales["en-AU"],
            (
                "en",
                {
                    ("short", "meter"): {
                        "one": "{0} metre",
                        "other": "{0} metres",
                    }
                },
            ),
        )
        # The root lists everything, and its parent is "-".
        self.assertEqual(locales["und"][0], "-")
        self.assertIn(("short", "kilometer"), locales["und"][1])
        # en equals und.
        self.assertEqual(locales["en"], ("und", {}))
        # ar has no entries of its own for what und has.
        self.assertEqual(locales["ar"], ("und", {}))

    def test_units_string_table(self):
        strings, locales = self.units()
        self.assertEqual(len(strings), len(set(strings)))
        self.assertEqual(strings.count("{0} km"), 1)
        und = locales["und"][1]
        self.assertEqual(und[("short", "kilometer")]["one"], "{0} km")
        self.assertEqual(und[("short", "kilometer")]["other"], "{0} km")
        # The same pattern in two units (and keys) is one string.
        lines = gen.units_lines(self.tree(), ["en", "en-AU", "ar"], SANCTIONED)
        self.assertEqual(lines.count("{0} km"), 1)
        self.assertEqual(und[("short", "-")], {"per": "{0}/{1}"})
        self.assertEqual(und[("long", "meter")]["other"], "{0} meters")
        # The count line is exact.
        self.assertEqual(lines[0].split(" "), ["strings", str(len(strings))])

    def test_units_compound_filter(self):
        _, locales = self.units()
        und = locales["und"][1]
        self.assertIn(("short", "kilometer-per-hour"), und)
        self.assertNotIn(("short", "foo-per-hour"), und)
        self.assertNotIn(("short", "mile-per-gallon-imperial"), und)
        units_seen = {u for _, u in und}
        self.assertEqual(
            units_seen, {"kilometer", "meter", "kilometer-per-hour", "-"}
        )

    def test_missing_locale_uses_likely_script(self):
        n = dict(FIXTURE_NUMBERS)
        n["zh-Hant-HK"] = numbers(("latn", ".", "#,##0%"))
        n["zh-Hans-SG"] = numbers(("latn", ".", "#,##0 %"))
        u = dict(FIXTURE_UNITS)
        u["zh-Hant-HK"] = u["en"]
        u["zh-Hans-SG"] = u["en"]
        tree = self.tree(n, u)
        lines = gen.number_symbols_lines(tree, ["zh-HK", "zh-SG"])
        # By the tag's own likelySubtags entry.
        self.assertIn("zh-HK\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t\t:", lines)
        # By the script of its language, when the tag has no entry.
        self.assertIn("zh-SG\tlatn\t%\tE\t∞\tNaN\t~\t#,##0 %\t{0}–{1}\t\t\t:", lines)
        _, locales = parse_units(
            gen.units_lines(tree, ["zh-HK", "zh-SG"], SANCTIONED)
        )
        self.assertIn("zh-HK", locales)
        self.assertIn("zh-SG", locales)
        self.assertNotIn("zh-Hant-HK", locales)

    def test_missing_locale_that_cannot_resolve_fails(self):
        tree = self.tree()
        # zz has a likely script, but zz-Latn-ZZ has no directory either.
        with self.assertRaisesRegex(SystemExit, "zz"):
            gen.number_symbols_lines(tree, ["zz"])
        with self.assertRaisesRegex(SystemExit, "qq"):
            gen.units_lines(tree, ["qq"], SANCTIONED)

    def test_rejects_tabs(self):
        n = dict(FIXTURE_NUMBERS)
        n["en"] = numbers(("latn", ".", "#,##0\t%"))
        with self.assertRaisesRegex(SystemExit, "tab"):
            gen.number_symbols_lines(self.tree(n), ["en"])
        u = dict(FIXTURE_UNITS)
        u["en"] = units(short={"length-meter": unit("{0}\tm", "{0} m")})
        with self.assertRaisesRegex(SystemExit, "tab"):
            gen.units_lines(self.tree(unit_data=u), ["en"], SANCTIONED)
        n["en"] = numbers(("latn", ".", "#,##0%\n"))
        with self.assertRaisesRegex(SystemExit, "newline"):
            gen.number_symbols_lines(self.tree(n), ["en"])

    def test_sanctioned_units_come_from_the_js_list(self):
        js = "var SANCTIONED_UNITS = [\n  'acre', 'bit',\n  'fluid-ounce',\n];\n"
        self.assertEqual(gen.sanctioned_units(js), ["acre", "bit", "fluid-ounce"])
        with self.assertRaises(SystemExit):
            gen.sanctioned_units("var X = [];")

    def test_release_tag_to_npm_version(self):
        self.assertEqual(gen.npm_version("release-48-2"), "48.2.0")
        self.assertEqual(gen.npm_version("release-49"), "49.0.0")
        with self.assertRaises(SystemExit):
            gen.npm_version("48.2")


ROOT_XML = """<ldml><numbers>
<defaultNumberingSystem>latn</defaultNumberingSystem>
<symbols numberSystem="arab">
  <decimal>٫</decimal><group>٬</group><list>؛</list>
  <percentSign>٪؜</percentSign><plusSign>؜+</plusSign><minusSign>؜-</minusSign>
  <approximatelySign>~</approximatelySign><exponential>اس</exponential>
  <superscriptingExponent>×</superscriptingExponent><perMille>؉</perMille>
  <infinity>∞</infinity><nan>NaN</nan><timeSeparator>:</timeSeparator>
</symbols>
<symbols numberSystem="beng">
  <alias source="locale" path="../symbols[@numberSystem='latn']"/>
</symbols>
<symbols numberSystem="latn"><decimal>.</decimal></symbols>
<decimalFormats numberSystem="arab">
  <alias source="locale" path="../decimalFormats[@numberSystem='latn']"/>
</decimalFormats>
<percentFormats numberSystem="arab">
  <percentFormatLength><percentFormat><pattern>#,##0%</pattern></percentFormat>
  </percentFormatLength>
</percentFormats>
<percentFormats numberSystem="arabext">
  <alias source="locale" path="../percentFormats[@numberSystem='latn']"/>
</percentFormats>
<currencyFormats numberSystem="arab">
  <currencySpacing>
    <alias source="locale" path="../../currencyFormats[@numberSystem='latn']/currencySpacing"/>
  </currencySpacing>
  <currencyFormatLength>
    <currencyFormat type="standard">
      <pattern>#,##0.00\u00a0¤</pattern>
      <pattern alt="noCurrency">#,##0.00</pattern>
    </currencyFormat>
    <currencyFormat type="accounting">
      <alias source="locale" path="../currencyFormat[@type='standard']"/>
    </currencyFormat>
  </currencyFormatLength>
</currencyFormats>
<miscPatterns numberSystem="arab">
  <alias source="locale" path="../miscPatterns[@numberSystem='latn']"/>
</miscPatterns>
</numbers></ldml>"""

EN_XML = """<ldml><numbers>
<symbols numberSystem="arab">
  <decimal>↑↑↑</decimal><plusSign>\u200f+</plusSign>
  <minusSign draft="unconfirmed">x</minusSign>
</symbols>
<symbols numberSystem="bali"><nan>nb</nan></symbols>
<symbols numberSystem="latn"><nan>NaN</nan></symbols>
<percentFormats numberSystem="arab">
  <percentFormatLength><percentFormat><pattern>#,##0\u00a0%</pattern></percentFormat>
  </percentFormatLength>
</percentFormats>
<miscPatterns numberSystem="arab">
  <pattern type="atLeast">{0}+</pattern><pattern type="range">{0}~{1}</pattern>
</miscPatterns>
<currencyFormats numberSystem="arab">
  <currencyFormatLength>
    <currencyFormat type="standard">
      <pattern>¤#,##0.00</pattern>
      <pattern alt="alphaNextToNumber">¤\u00a0#,##0.00</pattern>
    </currencyFormat>
  </currencyFormatLength>
  <currencyFormatLength type="short">
    <currencyFormat type="standard">
      <pattern type="1000" count="other">¤0K</pattern>
    </currencyFormat>
  </currencyFormatLength>
</currencyFormats>
</numbers></ldml>"""

# ar has arab data in cldr-json, so ar.xml's arab entries are left out.
AR_XML = """<ldml><numbers>
<symbols numberSystem="arab"><nan>ليس</nan></symbols>
<symbols numberSystem="arabext"><nan>ليس رقمًا</nan></symbols>
</numbers></ldml>"""


class ExplicitNumberSystemsTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tree = make_tree(
            self._tmp.name,
            FIXTURE_NUMBERS,
            FIXTURE_UNITS,
            FIXTURE_PARENTS,
            FIXTURE_LIKELY,
            FIXTURE_CURRENCIES,
        )

    def lines(self, xml=None):
        files = {"root": ROOT_XML, "en": EN_XML, "ar": AR_XML}
        files.update(xml or {})
        return gen.explicit_number_systems_lines(
            self.tree,
            ["en", "en-AU", "ar"],
            lambda name: ET.fromstring(files.get(name, "<ldml/>")),
        )

    def test_explicit_entries_along_the_chains(self):
        e = ""
        self.assertEqual(
            self.lines(),
            [
                "locale\tund\t-",
                "\t".join(
                    ["symbols", "arab", "٫", "٬", "؜-", "؜+", "٪؜", "اس"]
                    + ["∞", "NaN", "~", ":"]
                ),
                "percent\tarab\t#,##0%",
                "currency\tarab\t#,##0.00\u00a0¤\t\t=standard\t",
                "locale\tar\tund",
                "\t".join(
                    ["symbols", "arabext", e, e, e, e, e, e, e, "ليس رقمًا", e, e]
                ),
                "locale\ten\tund",
                "\t".join(["symbols", "arab", e, e, e, "\u200f+", e, e, e, e, e, e]),
                "\t".join(["symbols", "bali", e, e, e, e, e, e, e, "nb", e, e]),
                "percent\tarab\t#,##0\u00a0%",
                "range\tarab\t{0}~{1}",
                "currency\tarab\t¤#,##0.00\t¤\u00a0#,##0.00\t\t",
                "locale\ten-001\ten",
                "locale\ten-AU\ten-001",
            ],
        )

    def test_unhandled_data_fails(self):
        alias = (
            '<ldml><numbers><symbols numberSystem="beng">'
            "<alias source=\"locale\" path=\"../symbols[@numberSystem='latn']\"/>"
            "</symbols></numbers></ldml>"
        )
        with self.assertRaisesRegex(
            SystemExit, "en symbols beng: an alias outside root"
        ):
            self.lines({"en": alias})
        decimal = ROOT_XML.replace(
            """<decimalFormats numberSystem="arab">
  <alias source="locale" path="../decimalFormats[@numberSystem='latn']"/>
</decimalFormats>""",
            '<decimalFormats numberSystem="arab"><decimalFormatLength><decimalFormat>'
            "<pattern>#,##0.###</pattern></decimalFormat></decimalFormatLength>"
            "</decimalFormats>",
        )
        with self.assertRaisesRegex(SystemExit, "grouping"):
            self.lines({"root": decimal})

    def test_alternates_and_currency_separators(self):
        # A symbol's alt variant is not the symbol.
        alt = EN_XML.replace(
            "<plusSign>\u200f+</plusSign>",
            '<plusSign alt="us">ALT</plusSign><plusSign>\u200f+</plusSign>',
        )
        self.assertEqual(self.lines({"en": alt}), self.lines())
        only_alt = EN_XML.replace(
            "<plusSign>\u200f+</plusSign>", '<plusSign alt="us">ALT</plusSign>'
        )
        self.assertNotIn("ALT", "".join(self.lines({"en": only_alt})))
        # Currency separators of such a numbering system are not handled.
        group = EN_XML.replace(
            "<decimal>↑↑↑</decimal>", "<currencyGroup>.</currencyGroup>"
        )
        with self.assertRaisesRegex(SystemExit, "currencyGroup"):
            self.lines({"en": group})

    def test_time_separator_column(self):
        # A locale's timeSeparator is the last column of its rows of
        # number-symbols.txt.
        n = {t: dict(v) for t, v in FIXTURE_NUMBERS.items()}
        n["en"] = numbers(("latn", ".", "#,##0%"))
        n["en"]["symbols-numberSystem-latn"]["timeSeparator"] = "."
        n["ar"] = numbers(("arab", "٫", "#,##0‏%"), ("latn", ".", "#,##0%"))
        n["ar"]["symbols-numberSystem-arab"]["timeSeparator"] = "٫"
        tree = make_tree(
            tempfile.mkdtemp(dir=self._tmp.name),
            n,
            FIXTURE_UNITS,
            FIXTURE_PARENTS,
            FIXTURE_LIKELY,
            FIXTURE_CURRENCIES,
        )
        rows = {
            tuple(l.split("\t")[:2]): l.split("\t")
            for l in gen.number_symbols_lines(tree, ["en", "ar"])
        }
        self.assertEqual(rows[("en", "latn")][-1], ".")
        self.assertEqual(rows[("ar", "arab")][-1], "٫")
        self.assertEqual(rows[("ar", "latn")][-1], ":")
        self.assertEqual(rows[("und", "latn")][-1], ":")
        self.assertEqual({len(r) for r in rows.values()}, {12})
        # A locale without one fails.
        del n["en"]["symbols-numberSystem-latn"]["timeSeparator"]
        tree = make_tree(
            tempfile.mkdtemp(dir=self._tmp.name),
            n,
            FIXTURE_UNITS,
            FIXTURE_PARENTS,
            FIXTURE_LIKELY,
            FIXTURE_CURRENCIES,
        )
        with self.assertRaisesRegex(SystemExit, "en latn: missing 'timeSeparator'"):
            gen.number_symbols_lines(tree, ["en"])
        # An explicit root timeSeparator for a numbering system reaches
        # explicit-number-systems.txt, as the last symbol.
        root = ROOT_XML.replace(
            '<symbols numberSystem="latn">',
            '<symbols numberSystem="arabext"><timeSeparator>٫</timeSeparator>'
            '</symbols><symbols numberSystem="latn">',
        )
        e = ""
        lines = self.lines({"root": root})
        self.assertIn("\t".join(["symbols", "arabext"] + [e] * 9 + ["٫"]), lines)
        arab = [l for l in lines if l.startswith("symbols\tarab\t")][0]
        self.assertEqual(arab.split("\t")[-1], ":")

    def test_xml_file_names(self):
        self.assertEqual(gen.xml_file_name("und"), "root")
        self.assertEqual(gen.xml_file_name("zh-Hant-HK"), "zh_Hant_HK")
        self.assertEqual(gen.xml_file_name("be-tarask"), "be_TARASK")
        self.assertEqual(gen.xml_file_name("ca-ES-valencia"), "ca_ES_VALENCIA")
        self.assertEqual(gen.xml_file_name("en-001"), "en_001")


def short_currency(patterns):
    """cldr-json's short currency formats object from {key: pattern}, the
    keys as cldr-json has them ("1000-count-one")."""
    return {"short": {"standard": dict(patterns)}}


def magnitudes(prefix, suffix, counts=("one", "other"), alt=None, low=3, high=5):
    """Short currency keys for 10^low to 10^high: prefix, the digits (one
    more each magnitude, from one), suffix; and the alternate with alt in
    place of prefix."""
    out = {}
    for n in range(low, high + 1):
        digits = "0" * (n - low + 1)
        for c in counts:
            out[f"1{'0' * n}-count-{c}"] = prefix + digits + suffix
            if alt is not None:
                key = f"1{'0' * n}-count-{c}-alt-alphaNextToNumber"
                out[key] = alt + digits + suffix
    return out


ROOT_SHORT = magnitudes("¤\u00a0", "K", counts=("other",))
EN_SHORT = magnitudes("¤", "K", alt="¤\u00a0")
COMPACT_NUMBERS = {
    "und": {"currencyFormats-numberSystem-latn": short_currency(ROOT_SHORT)},
    "en": {"currencyFormats-numberSystem-latn": short_currency(EN_SHORT)},
    "en-AU": {"currencyFormats-numberSystem-latn": short_currency(EN_SHORT)},
    # ar's arab currency formats have no short ones; latn's are its own.
    "ar": {
        "currencyFormats-numberSystem-arab": {"standard": "#,##0.00\u00a0¤"},
        "currencyFormats-numberSystem-latn": short_currency(
            {
                **magnitudes("\u200f", "\u00a0ألف\u00a0¤"),
                # A 10^6 pattern of "0" has no compact form.
                "1000000-count-one": "0",
                "1000000-count-other": "0",
            }
        ),
    },
}


def parse_compact_currency(lines):
    """Parse compact_currency_lines: {tag: (parent, {(nu, n, count):
    (pattern, alternate)})}, with each pattern's digits restored ("" for no
    alternate)."""
    head, count = lines[0].split(" ")
    assert head == "strings"
    strings = lines[1 : 1 + int(count)]
    out = {}
    for line in lines[1 + int(count) :]:
        f = line.split("\t")
        if f[0] == "locale":
            cur = out[f[1]] = (f[2], {})
            continue
        nu, span, zeros, items = f
        first, _, last = span.partition("-")
        first, last = int(first), int(last or first)
        for n in range(first, last + 1):
            z = int(zeros) + (n - first if int(zeros) else 0)
            for item in items.split(" "):
                c, refs = item.split("=")
                plain, _, alt = refs.partition(",")
                delta = 0
                for sign in "+-":
                    if sign in alt:
                        alt, d = alt.split(sign)
                        delta = int(sign + d)

                def digits(i, zz):
                    s = strings[int(i)]
                    return s if s == "0" else s.replace("0", "0" * zz, 1)

                cur[1][(nu, n, c)] = (
                    digits(plain, z),
                    digits(alt, z + delta) if alt else "",
                )
    return out


class CompactCurrencyTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)

    def lines(self, number_data=None, xml=None):
        tree = make_tree(
            tempfile.mkdtemp(dir=self._tmp.name),
            COMPACT_NUMBERS if number_data is None else number_data,
            FIXTURE_UNITS,
            FIXTURE_PARENTS,
            FIXTURE_LIKELY,
            FIXTURE_CURRENCIES,
        )
        files = {"root": ROOT_XML, "en": EN_XML, "ar": AR_XML}
        files.update(xml or {})
        return gen.compact_currency_lines(
            tree,
            ["en", "en-AU", "ar"],
            lambda name: ET.fromstring(files.get(name, "<ldml/>")),
        )

    def test_entries_differ_from_the_parent_chain(self):
        lines = self.lines()
        locales = parse_compact_currency(lines)
        und = locales["und"]
        self.assertEqual(und[0], "-")
        self.assertEqual(
            {k: v for k, v in und[1].items() if k[1] == 3},
            {("latn", 3, "other"): ("¤\u00a00K", "")},
        )
        en = locales["en"][1]
        self.assertEqual(en[("latn", 4, "one")], ("¤00K", "¤\u00a000K"))
        self.assertEqual(en[("latn", 5, "other")], ("¤000K", "¤\u00a0000K"))
        # en-AU's patterns are en's.
        self.assertEqual(locales["en-AU"], ("en", {}))
        # ar's 10^6 "0" pattern; its arab ones are its latn ones, so the
        # file lists nothing for arab.
        ar = locales["ar"][1]
        self.assertEqual(ar[("latn", 6, "one")], ("0", ""))
        self.assertEqual(ar[("latn", 3, "one")], ("\u200f0\u00a0ألف\u00a0¤", ""))
        self.assertFalse([k for k in ar if k[0] == "arab"])
        # The strings are shared and hold one 0 for the digits.
        strings = lines[1 : 1 + int(lines[0].split(" ")[1])]
        self.assertEqual(len(strings), len(set(strings)))
        self.assertIn("¤0K", strings)
        self.assertNotIn("¤00K", strings)
        # Consecutive magnitudes whose digits grow by one fold into a line.
        self.assertIn("latn\t3-5\t1\tone=", "\n".join(lines))

    def test_explicit_short_formats_of_another_numbering_system(self):
        # en.xml has arab's 10^3 "other" pattern, which is en's latn one, so
        # nothing is listed for arab.
        en = parse_compact_currency(self.lines())["en"][1]
        self.assertFalse([k for k in en if k[0] == "arab"])
        # One that differs is listed alone, with latn's alternate; en's
        # other arab patterns are its latn ones.
        xml = EN_XML.replace("¤0K</pattern>", "¤0\u00a0K</pattern>")
        en = parse_compact_currency(self.lines(xml={"en": xml}))["en"][1]
        arab = {k: v for k, v in en.items() if k[0] == "arab"}
        self.assertEqual(arab, {("arab", 3, "other"): ("¤0\u00a0K", "¤\u00a00K")})
        # An explicit alternate whose digits differ from the pattern's.
        xml = EN_XML.replace(
            '<pattern type="1000" count="other">¤0K</pattern>',
            '<pattern type="1000" count="other">¤0K</pattern>'
            '<pattern type="1000" count="other" alt="alphaNextToNumber">¤000L</pattern>'
            '<pattern type="1000" count="one" alt="alphaNextToNumber">¤000L</pattern>',
        )
        lines = self.lines(xml={"en": xml})
        self.assertIn("arab\t3\t1\tone=", "\n".join(lines))
        en = parse_compact_currency(lines)["en"][1]
        self.assertEqual(en[("arab", 3, "other")], ("¤0K", "¤000L"))
        self.assertEqual(en[("arab", 3, "one")], ("¤0K", "¤000L"))

    def test_shapes(self):
        self.assertEqual(gen.short_currency_shape("¤00K", "w"), ("¤0K", 2))
        self.assertEqual(gen.short_currency_shape("0", "w"), ("0", 0))
        # Quoted text is kept; a negative subpattern is dropped.
        self.assertEqual(
            gen.short_currency_shape("0\u00a0Mio'.'\u00a0¤", "w"),
            ("0\u00a0Mio'.'\u00a0¤", 1),
        )
        self.assertEqual(
            gen.short_currency_shape("¤\u00a0elfu\u00a000;¤\u00a0elfu\u00a0-00", "w"),
            ("¤\u00a0elfu\u00a00", 2),
        )
        # A letter is text (hu's "E" for ezer).
        self.assertEqual(gen.short_currency_shape("0 E ¤", "w"), ("0 E ¤", 1))
        for bad in ["¤0K¤", "0K", "¤0K0", "K¤ 0", "¤#K", "¤0.0K", "¤0%"]:
            with self.assertRaises(SystemExit, msg=bad):
                gen.short_currency_shape(bad, "w")

    def test_unhandled_data_fails(self):
        n = dict(COMPACT_NUMBERS)
        # Counts of one magnitude with different digits.
        n["en"] = {
            "currencyFormats-numberSystem-latn": short_currency(
                {**EN_SHORT, "1000-count-one": "¤00K"}
            )
        }
        with self.assertRaisesRegex(SystemExit, "digits differ"):
            self.lines(n)
        n["en"] = {
            "currencyFormats-numberSystem-latn": short_currency(
                {**EN_SHORT, "1000-count-1": "¤0K"}
            )
        }
        with self.assertRaisesRegex(SystemExit, "unexpected short currency key"):
            self.lines(n)
        # A locale that drops a magnitude its parent has cannot be read back.
        n["en"] = COMPACT_NUMBERS["en"]
        n["en-AU"] = {
            "currencyFormats-numberSystem-latn": short_currency(
                {k: v for k, v in EN_SHORT.items() if not k.startswith("100000-")}
            )
        }
        with self.assertRaisesRegex(SystemExit, "does not give it back"):
            self.lines(n)
        # Root's short currency formats for a numbering system other than
        # latn.
        root = ROOT_XML.replace(
            "</currencyFormats>",
            '<currencyFormatLength type="short"><currencyFormat type="standard">'
            '<pattern type="1000" count="other">¤0K</pattern></currencyFormat>'
            "</currencyFormatLength></currencyFormats>",
            1,
        )
        with self.assertRaisesRegex(SystemExit, "root currencyFormats arab"):
            self.lines(xml={"root": root})


# The calendars of the fixture date packages: (file name, package), as the
# CLDR JSON npm packages have them.
DATE_FILES = [
    ("gregorian", "cldr-dates-full"),
    ("generic", "cldr-dates-full"),
    ("buddhist", "cldr-cal-buddhist-full"),
    ("chinese", "cldr-cal-chinese-full"),
    ("coptic", "cldr-cal-coptic-full"),
    ("dangi", "cldr-cal-dangi-full"),
    ("ethiopic-amete-alem", "cldr-cal-ethiopic-full"),
    ("ethiopic", "cldr-cal-ethiopic-full"),
    ("hebrew", "cldr-cal-hebrew-full"),
    ("indian", "cldr-cal-indian-full"),
    ("islamic-civil", "cldr-cal-islamic-full"),
    ("islamic-tbla", "cldr-cal-islamic-full"),
    ("islamic-umalqura", "cldr-cal-islamic-full"),
    ("japanese", "cldr-cal-japanese-full"),
    ("persian", "cldr-cal-persian-full"),
    ("roc", "cldr-cal-roc-full"),
]
WIDTH_NAMES = ["full", "long", "medium", "short"]


def calendar_json(entries, day_periods=None):
    """The calendar object of a ca-*.json file from date-patterns.txt keys
    and their patterns (`entries`), with the format dayPeriods names
    {width: {period: name}}."""
    j = {
        "months": {},
        "dateFormats": {},
        "dateSkeletons": {"full": "yMMMMEEEEd"},
        "timeFormats": {},
        "dateTimeFormats": {
            "availableFormats": {},
            "appendItems": {"Day": "{0} ({2}: {1})", "Era": "{0} {1}"},
            "intervalFormats": {},
        },
        "dateTimeFormats-atTime": {"standard": {}},
        "dateTimeFormats-relative": {"standard": {"full": "{1} 'at' {0}"}},
        "dayPeriods": {"format": day_periods or {}, "stand-alone": {}},
    }
    dt = j["dateTimeFormats"]
    sections = {
        "d": j["dateFormats"],
        "t": j["timeFormats"],
        "c": dt,
        "a": j["dateTimeFormats-atTime"]["standard"],
    }
    for k, v in entries.items():
        if len(k) == 2 and k[0] in sections and k[1].isdigit():
            sections[k[0]][WIDTH_NAMES[int(k[1])]] = v
        elif k == "fb":
            dt["intervalFormats"]["intervalFormatFallback"] = v
        elif k == "tz":
            dt["appendItems"]["Timezone"] = v
        elif k == "lp":
            j["monthPatterns"] = {
                "format": {"wide": {"leap": "{0} leap"}},
                "numeric": {"all": {"leap": v}},
            }
        elif k.startswith("f."):
            dt["availableFormats"][k[2:]] = v
        elif k.startswith("i."):
            skeleton, letter = k[2:].split(".")
            dt["intervalFormats"].setdefault(skeleton, {})[letter] = v
        else:
            raise ValueError(k)
    return j


def widths(*values):
    return {f"{p}{i}": v for p, vs in values for i, v in enumerate(vs)}


UND_GENERIC = {
    **widths(
        ("d", ["G y MMMM d, EEEE", "G y MMMM d", "G y MMM d", "GGGGG y-MM-dd"]),
        ("t", ["HH:mm:ss zzzz", "HH:mm:ss z", "HH:mm:ss", "HH:mm"]),
        ("c", ["{1} {0}"] * 4),
        ("a", ["{1} {0}"] * 4),
    ),
    "fb": "{0} – {1}",
    "tz": "{0} {1}",
    "f.yMMMd": "G y MMM d",
    "f.Hm": "HH:mm",
    "i.yMMMd.d": "G y MMM d–d",
    "i.yMMMd.M": "G y MMM d – MMM d",
}
UND_GREGORIAN = {
    **UND_GENERIC,
    **widths(("d", ["y MMMM d, EEEE", "y MMMM d", "y MMM d", "y-MM-dd"])),
    "f.yMMMd": "y MMM d",
    "i.yMMMd.d": "y MMM d–d",
    "i.yMMMd.M": "y MMM d – MMM d",
}
EN_GREGORIAN = {
    **widths(
        ("d", ["EEEE, MMMM d, y", "MMMM d, y", "MMM d, y", "M/d/yy"]),
        ("t", ["h:mm:ss a zzzz", "h:mm:ss a z", "h:mm:ss a", "h:mm a"]),
        ("c", ["{1}, {0}"] * 4),
        ("a", ["{1} 'at' {0}", "{1} 'at' {0}", "{1}, {0}", "{1}, {0}"]),
    ),
    "fb": "{0} – {1}",
    "tz": "{0} {1}",
    "f.yMMMd": "MMM d, y",
    "f.Hm": "HH:mm",
    "i.yMMMd.d": "MMM d – d, y",
    "i.yMMMd.M": "MMM d – MMM d, y",
}
EN_GENERIC = {
    **EN_GREGORIAN,
    **widths(("d", ["EEEE, MMMM d, y G", "MMMM d, y G", "MMM d, y G", "M/d/y GGGGG"])),
    "f.yMMMd": "MMM d, y G",
    "i.yMMMd.d": "MMM d – d, y G",
    "i.yMMMd.M": "MMM d – MMM d, y G",
}
EN_AU_GREGORIAN = {**EN_GREGORIAN, "d3": "d/M/yy"}
EN_AU_GENERIC = {**EN_GENERIC, "d3": "d/M/y GGGGG"}
# en-AU's buddhist has a pattern of its own; en's is its generic.
EN_AU_BUDDHIST = {**EN_AU_GENERIC, "d1": "d MMMM y G"}

# CLDR entries the generator leaves out, added to en's gregorian.
EN_DROPPED = {
    "timeFormats": {"short-alt-ascii": "h:mm a", "short-alt-variant": "H:mm"},
    "dateFormats": {"short-alt-variant": "MM/dd/yy"},
    "availableFormats": {
        "yMMMd-alt-ascii": "MMM d, y",
        "Hm-alt-variant": "H:mm",
        "yw-count-one": "'week' w 'of' Y",
        "yw": "'week' w 'of' Y",
        "MMMMW-count-other": "'week' W 'of' MMMM",
        "yQQQ": "QQQ y",
        "Ed-alt-variant": "d e",
        "ed": "d e",
    },
    "intervalFormats": {"yMMMd": {"d-alt-variant": "MMM d-d, y"}, "yw": {"y": "w y"}},
}

# Names of day periods (the format context), {width: {period: name}}.
NAMES_UND = {w: {"am": "AM", "pm": "PM"} for w in ("abbreviated", "wide", "narrow")}
EN_NAMES = {
    w: {
        "am": "AM",
        "pm": "PM",
        "midnight": {"abbreviated": "midnight", "wide": "at midnight"}.get(w, "mi"),
        "noon": "noon" if w != "narrow" else "n",
        "morning1": "in the morning",
        "afternoon1": "in the afternoon",
        "evening1": "in the evening",
        "night1": "at night",
    }
    for w in ("abbreviated", "wide", "narrow")
}
EN_AU_NAMES = {w: dict(v) for w, v in EN_NAMES.items()}
EN_AU_NAMES["narrow"]["morning1"] = "morning"
EN_AU_NAMES["narrow"]["noon"] = "noon"


def date_locale(gregorian, generic, names, **calendars):
    """The calendars of a fixture locale: each one not given is its
    generic, with chinese and dangi given the leap month pattern."""
    out = {"gregorian": gregorian, "generic": generic}
    for name, _ in DATE_FILES:
        if name not in out:
            out[name] = calendars.get(name)
            if out[name] is None:
                out[name] = dict(generic)
                if name in ("chinese", "dangi"):
                    out[name]["lp"] = "{0}bis"
    return (out, names)


FIXTURE_DATES = {
    "und": date_locale(UND_GREGORIAN, UND_GENERIC, NAMES_UND),
    "en": date_locale(EN_GREGORIAN, EN_GENERIC, EN_NAMES),
    "en-AU": date_locale(
        EN_AU_GREGORIAN, EN_AU_GENERIC, EN_AU_NAMES, buddhist=EN_AU_BUDDHIST
    ),
}

# The day period rules of dayPeriods.json: en's CLDR 48.2 ones; bg's
# night1 crosses midnight; es_CO's are its own (written with an underscore,
# as dayPeriods.xml has it); und's are am and pm.
FIXTURE_RULES = {
    "und": {
        "am": {"_from": "00:00", "_before": "12:00"},
        "pm": {"_from": "12:00", "_before": "24:00"},
    },
    "en": {
        "midnight": {"_at": "00:00"},
        "noon": {"_at": "12:00"},
        "morning1": {"_from": "00:00", "_before": "12:00"},
        "afternoon1": {"_from": "12:00", "_before": "18:00"},
        "evening1": {"_from": "18:00", "_before": "21:00"},
        "night1": {"_from": "21:00", "_before": "24:00"},
    },
    "bg": {
        "morning1": {"_from": "04:00", "_before": "11:00"},
        "morning2": {"_from": "11:00", "_before": "14:00"},
        "afternoon1": {"_from": "14:00", "_before": "18:00"},
        "evening1": {"_from": "18:00", "_before": "22:00"},
        "night1": {"_from": "22:00", "_before": "04:00"},
    },
    "es": {
        "noon": {"_at": "12:00"},
        "morning1": {"_from": "00:00", "_before": "06:00"},
        "morning2": {"_from": "06:00", "_before": "12:00"},
        "evening1": {"_from": "12:00", "_before": "20:00"},
        "night1": {"_from": "20:00", "_before": "24:00"},
    },
    "es_CO": {
        "noon": {"_at": "12:00"},
        "morning2": {"_from": "00:00", "_before": "12:00"},
        "evening1": {"_from": "12:00", "_before": "20:00"},
        "night1": {"_from": "20:00", "_before": "24:00"},
    },
    "zh": {
        "morning1": {"_from": "00:00", "_before": "12:00"},
        "afternoon1": {"_from": "12:00", "_before": "24:00"},
    },
    "lij": {
        "morning1": {"_from": "00:00", "_before": "12:00"},
        "afternoon1": {"_from": "12:00", "_before": "24:00"},
    },
}


def names_of(periods):
    """Format dayPeriods names with each width's name of each period of
    {period: name} the same."""
    return {w: dict(periods) for w in ("abbreviated", "wide", "narrow")}


ES_NAMES = names_of(
    {
        "am": "a. m.",
        "pm": "p. m.",
        "noon": "del mediodía",
        "morning1": "de la madrugada",
        "morning2": "de la mañana",
        "evening1": "de la tarde",
        "night1": "de la noche",
    }
)
ZH_NAMES = names_of({"am": "上午", "pm": "下午", "morning1": "早", "afternoon1": "晚"})
ZH_HANT_NAMES = names_of(
    {"am": "上午", "pm": "下午", "morning1": "朝", "afternoon1": "夜"}
)


def generic_locale(names):
    return date_locale(UND_GREGORIAN, UND_GENERIC, names)


# Locales of the day period tests, with their names; zh-Hant's CLDR parent
# is root, so its parent in the file is und while its rules are zh's.
DAY_PERIOD_DATES = {
    "und": generic_locale(NAMES_UND),
    "en": generic_locale(EN_NAMES),
    "en-AU": generic_locale(EN_AU_NAMES),
    "es": generic_locale(ES_NAMES),
    "es-CO": generic_locale(ES_NAMES),
    "zh": generic_locale(ZH_NAMES),
    "zh-Hant": generic_locale(ZH_HANT_NAMES),
    "zh-Hant-HK": generic_locale(ZH_HANT_NAMES),
    "lij": generic_locale(NAMES_UND),
}
DAY_PERIOD_TAGS = ["en", "en-AU", "es", "es-CO", "zh", "zh-Hant", "zh-HK", "lij"]
DATE_PARENTS = {"en-AU": "en-001", "zh-Hant": "root"}
DATE_LIKELY = {"zh-HK": "zh-Hant-HK", "zh": "zh-Hans-CN"}

# root.xml's iso8601 calendar, as CLDR 48.2 has it, shortened.
ISO8601_XML = """<calendar type="iso8601">
<months><alias source="locale" path="../../calendar[@type='gregorian']/months"/></months>
<dateFormats>
<dateFormatLength type="full"><dateFormat><pattern>y MMMM d, EEEE</pattern>
<datetimeSkeleton>yMMMMEEEEd</datetimeSkeleton></dateFormat></dateFormatLength>
<dateFormatLength type="long"><dateFormat><pattern>y MMMM d</pattern></dateFormat></dateFormatLength>
<dateFormatLength type="medium"><dateFormat><pattern>y MMM d</pattern></dateFormat></dateFormatLength>
<dateFormatLength type="short"><dateFormat><pattern>y-MM-dd</pattern>
<pattern alt="variant">yy-MM-dd</pattern></dateFormat></dateFormatLength>
</dateFormats>
<timeFormats>
<timeFormatLength type="full"><timeFormat><pattern>HH:mm:ss zzzz</pattern></timeFormat></timeFormatLength>
<timeFormatLength type="long"><timeFormat><pattern>HH:mm:ss z</pattern></timeFormat></timeFormatLength>
<timeFormatLength type="medium"><timeFormat><pattern>HH:mm:ss</pattern></timeFormat></timeFormatLength>
<timeFormatLength type="short"><timeFormat><pattern>HH:mm</pattern></timeFormat></timeFormatLength>
</timeFormats>
<dateTimeFormats>
<dateTimeFormatLength type="full"><dateTimeFormat><pattern>{1} {0}</pattern></dateTimeFormat>
<dateTimeFormat type="atTime"><alias source="locale" path="../dateTimeFormat[@type='standard']"/></dateTimeFormat></dateTimeFormatLength>
<dateTimeFormatLength type="long"><dateTimeFormat><pattern>{1} {0}</pattern></dateTimeFormat>
<dateTimeFormat type="atTime"><alias source="locale" path="../dateTimeFormat[@type='standard']"/></dateTimeFormat></dateTimeFormatLength>
<dateTimeFormatLength type="medium"><dateTimeFormat><pattern>{1}, {0}</pattern></dateTimeFormat>
<dateTimeFormat type="atTime"><pattern>{1} 'at' {0}</pattern></dateTimeFormat></dateTimeFormatLength>
<dateTimeFormatLength type="short"><dateTimeFormat><pattern>{1} {0}</pattern></dateTimeFormat>
<dateTimeFormat type="atTime"><alias source="locale" path="../dateTimeFormat[@type='standard']"/></dateTimeFormat></dateTimeFormatLength>
<availableFormats>
<dateFormatItem id="yMMMd">y MMM d</dateFormatItem>
<dateFormatItem id="Hm">HH:mm</dateFormatItem>
<dateFormatItem id="yMMMd" alt="variant">y-MMM-d</dateFormatItem>
<dateFormatItem id="MMMMW" count="other">MMMM 'week' W</dateFormatItem>
<dateFormatItem id="yw" count="other">Y 'week' w</dateFormatItem>
<dateFormatItem id="yQQQ">y QQQ</dateFormatItem>
</availableFormats>
<appendItems>
<appendItem request="Day">{0} ({2}: {1})</appendItem>
<appendItem request="Timezone">{0} {1}</appendItem>
</appendItems>
<intervalFormats>
<intervalFormatFallback>{0} – {1}</intervalFormatFallback>
<intervalFormatItem id="yMMMd">
<greatestDifference id="d">y MMM d–d</greatestDifference>
<greatestDifference id="M">y MMM d – MMM d</greatestDifference>
<greatestDifference id="M" alt="variant">y MMM d - MMM d</greatestDifference>
</intervalFormatItem>
</intervalFormats>
</dateTimeFormats>
</calendar>"""
ROOT_DATES_XML = (
    "<ldml><dates><calendars><calendar type=\"gregorian\"/>"
    + ISO8601_XML
    + "</calendars></dates></ldml>"
)
ISO8601_ENTRIES = {
    **widths(
        ("d", ["y MMMM d, EEEE", "y MMMM d", "y MMM d", "y-MM-dd"]),
        ("t", ["HH:mm:ss zzzz", "HH:mm:ss z", "HH:mm:ss", "HH:mm"]),
        ("c", ["{1} {0}", "{1} {0}", "{1}, {0}", "{1} {0}"]),
        ("a", ["{1} {0}", "{1} {0}", "{1} 'at' {0}", "{1} {0}"]),
    ),
    "fb": "{0} – {1}",
    "tz": "{0} {1}",
    "f.yMMMd": "y MMM d",
    "f.Hm": "HH:mm",
    "i.yMMMd.d": "y MMM d–d",
    "i.yMMMd.M": "y MMM d – MMM d",
}

# The BCP 47 ids of the calendars of date-patterns.txt, by file name.
BCP47_CALENDAR = {
    "gregorian": "gregory",
    "ethiopic-amete-alem": "ethioaa",
}


def make_date_tree(root, locales, parents, likely, rules, dropped=None):
    """Write fixture cldr-dates-full, cldr-cal-*-full and cldr-core files
    under `root` for `locales` ({dir: (calendars, names)}, the calendars
    {file name: entries}), and return a tree reading them. `dropped` adds
    raw entries to en's gregorian."""
    for loc, (calendars, names) in locales.items():
        for name, package in DATE_FILES:
            j = calendar_json(calendars[name], names if name == "gregorian" else None)
            if loc == "en" and name == "gregorian" and dropped:
                for section, extra in dropped.items():
                    if section == "dateFormats":
                        j["dateFormats"].update(extra)
                    elif section == "timeFormats":
                        j["timeFormats"].update(extra)
                    elif section == "availableFormats":
                        j["dateTimeFormats"]["availableFormats"].update(extra)
                    else:
                        iv = j["dateTimeFormats"]["intervalFormats"]
                        for k, v in extra.items():
                            iv.setdefault(k, {}).update(v)
            write_json(
                root,
                [package, "package", "main", loc, f"ca-{name}.json"],
                {"main": {loc: {"dates": {"calendars": {name: j}}}}},
            )
    write_json(
        root,
        ["cldr-core", "package", "supplemental", "parentLocales.json"],
        {"supplemental": {"parentLocales": {"parentLocale": parents}}},
    )
    write_json(
        root,
        ["cldr-core", "package", "supplemental", "likelySubtags.json"],
        {"supplemental": {"likelySubtags": likely}},
    )
    write_json(
        root,
        ["cldr-core", "package", "supplemental", "dayPeriods.json"],
        {"supplemental": {"dayPeriodRuleSet": rules}},
    )
    return gen.Tree(root)


def parse_date_patterns(lines):
    """Parse date-patterns.txt lines: {tag: (parent, {calendar: {key:
    pattern} or ("=", other)})}."""
    lines = [l for l in lines if not l.startswith("#")]
    head, count = lines[0].split(" ")
    assert head == "strings"
    strings = lines[1 : 1 + int(count)]
    assert len(strings) == len(set(strings))
    out = {}
    for line in lines[1 + int(count) :]:
        f = line.split(" ")
        if f[0] == "locale":
            assert len(f) == 3, line
            cur = out[f[1]] = (f[2], {})
        elif f[1] == "=":
            assert len(f) == 3, line
            cur[1][f[0]] = ("=", f[2])
        else:
            assert f[0] not in cur[1], line
            cur[1][f[0]] = {
                k: strings[int(i)] for k, i in (kv.rsplit("=", 1) for kv in f[1:])
            }
    return out


def read_date_pair(parsed, tag, calendar):
    """(tag, calendar) of a parsed file, read as the brief says."""
    parent, calendars = parsed[tag]
    v = calendars.get(calendar)
    if isinstance(v, tuple):
        return read_date_pair(parsed, tag, v[1])
    base = {} if parent == "-" else read_date_pair(parsed, parent, calendar)
    return {**base, **(v or {})}


def parse_day_periods(lines):
    """Parse day-periods.txt lines: ({key: {period: (from, before)}},
    {tag: (parent, rules key, {period: (abbreviated, wide, narrow)})})."""
    rules, locales = {}, {}
    cur = None
    for line in lines:
        if line.startswith("#"):
            continue
        if line.startswith("rules "):
            assert not locales, line
            f = line.split(" ")
            assert len(f) == 2, line
            cur = rules[f[1]] = {}
        elif line.startswith("locale "):
            f = line.split(" ")
            assert len(f) == 4, line
            locales[f[1]] = (f[2], f[3], {})
            cur = locales[f[1]][2]
        elif not locales:
            period, start, before = line.split(" ")
            cur[period] = (int(start), int(before))
        else:
            period, names = line.split(" ", 1)
            names = tuple(names.split("\t"))
            assert len(names) == 3, line
            cur[period] = names
    return rules, locales


def day_period_names(locales, tag, period):
    """A period's names for `tag` along the parent chain, "" each if none."""
    while tag != "-":
        parent, _, names = locales[tag]
        if period in names:
            return names[period]
        tag = parent
    return ("", "", "")


class DatePatternsTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)

    def lines(self, locales=None, xml=None, tags=("en", "en-AU"), dropped=None):
        tree = make_date_tree(
            tempfile.mkdtemp(dir=self._tmp.name),
            FIXTURE_DATES if locales is None else locales,
            DATE_PARENTS,
            DATE_LIKELY,
            FIXTURE_RULES,
            EN_DROPPED if dropped is None else dropped,
        )
        files = {"root": ROOT_DATES_XML}
        files.update(xml or {})
        return gen.date_patterns_lines(
            tree,
            list(tags),
            lambda name: ET.fromstring(files.get(name, "<ldml/>")),
        )

    def parsed(self, **kw):
        return parse_date_patterns(self.lines(**kw))

    def test_date_pattern_keys(self):
        p = self.parsed()
        self.assertEqual(p["en"][0], "und")
        en = read_date_pair(p, "en", "gregory")
        self.assertEqual(en["d0"], "EEEE, MMMM d, y")
        self.assertEqual(en["d3"], "M/d/yy")
        self.assertEqual(en["t0"], "h:mm:ss a zzzz")
        self.assertEqual(en["t3"], "h:mm a")
        self.assertEqual(en["c0"], "{1}, {0}")
        self.assertEqual(en["a0"], "{1} 'at' {0}")
        self.assertEqual(en["a3"], "{1}, {0}")
        self.assertEqual(en["fb"], "{0} – {1}")
        self.assertEqual(en["tz"], "{0} {1}")
        self.assertEqual(en["f.yMMMd"], "MMM d, y")
        self.assertEqual(en["i.yMMMd.d"], "MMM d – d, y")
        self.assertNotIn("lp", en)
        self.assertEqual(read_date_pair(p, "en", "chinese")["lp"], "{0}bis")
        # The keys and calendars are exactly these.
        self.assertEqual(set(en), set(EN_GREGORIAN))
        cals = {c for _, (_, cs) in p.items() for c in cs}
        self.assertLessEqual(
            cals,
            {
                "gregory", "generic", "buddhist", "chinese", "coptic", "dangi",
                "ethioaa", "ethiopic", "hebrew", "indian", "islamic-civil",
                "islamic-tbla", "islamic-umalqura", "iso8601", "japanese",
                "persian", "roc",
            },
        )
        self.assertIn("ethioaa", p["und"][1])

    def test_date_pattern_calendar_alias(self):
        p = self.parsed()
        # en's buddhist is its generic; en-AU's has a pattern of its own.
        self.assertEqual(p["en"][1]["buddhist"], ("=", "generic"))
        self.assertEqual(p["en-AU"][1]["buddhist"], {"d1": "d MMMM y G", "d3": "d/M/y GGGGG"})
        # dangi equals the earlier chinese, which differs from generic.
        self.assertEqual(p["und"][1]["dangi"], ("=", "chinese"))
        self.assertIsInstance(p["und"][1]["chinese"], dict)
        # An alias is to a calendar listed before it.
        for tag, (_, cals) in p.items():
            order = list(cals)
            for cal, v in cals.items():
                if isinstance(v, tuple) and v[1] in cals:
                    self.assertLess(order.index(v[1]), order.index(cal), (tag, cal))
                    self.assertIsInstance(cals[v[1]], dict, (tag, cal))
        # Read through the alias.
        self.assertEqual(read_date_pair(p, "en", "buddhist"), EN_GENERIC)

    def test_date_pattern_dedup(self):
        p = self.parsed()
        # en-AU's gregorian differs from en's in d3 only.
        self.assertEqual(p["en-AU"][1]["gregory"], {"d3": "d/M/yy"})
        self.assertEqual(p["en-AU"][1]["generic"], {"d3": "d/M/y GGGGG"})
        # en lists what differs from und.
        self.assertNotIn("f.Hm", p["en"][1]["gregory"])
        self.assertEqual(p["en"][1]["gregory"]["f.yMMMd"], "MMM d, y")
        self.assertEqual(p["und"][0], "-")
        self.assertEqual(p["und"][1]["gregory"], UND_GREGORIAN)
        # A string used twice is one string.
        lines = self.lines()
        self.assertEqual(lines.count("{1} {0}"), 1)

    def test_date_pattern_reads_back(self):
        p = self.parsed()
        for tag, (calendars, _) in FIXTURE_DATES.items():
            for name, _ in DATE_FILES:
                cal = BCP47_CALENDAR.get(name, name)
                self.assertEqual(
                    read_date_pair(p, tag, cal), calendars[name], (tag, cal)
                )
            self.assertEqual(read_date_pair(p, tag, "iso8601"), ISO8601_ENTRIES, tag)
        # en-AU's gregorian lacks a key its parent en has.
        d = dict(FIXTURE_DATES)
        greg = {k: v for k, v in EN_AU_GREGORIAN.items() if k != "f.Hm"}
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        with self.assertRaisesRegex(SystemExit, "en-AU gregory"):
            self.lines(locales=d)

    def test_date_pattern_rejects_symbols(self):
        d = dict(FIXTURE_DATES)
        # A kept skeleton whose pattern has a quarter.
        greg = {**EN_AU_GREGORIAN, "f.yMMMd": "QQQ d, y"}
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        with self.assertRaisesRegex(SystemExit, "en-AU gregory.*QQQ d, y"):
            self.lines(locales=d)
        # A time format with a Z.
        greg = {**EN_AU_GREGORIAN, "t0": "h:mm:ss a Z"}
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        with self.assertRaisesRegex(SystemExit, "en-AU gregory.*h:mm:ss a Z"):
            self.lines(locales=d)
        # A skeleton with a w, Q or e is dropped, with its pattern.
        en = read_date_pair(self.parsed(), "en", "gregory")
        for k in ("f.yw", "f.yQQQ", "f.ed", "f.Ed", "i.yw.y"):
            self.assertNotIn(k, en)
        # A letter outside the table, even one ECMA-402 has no field for in
        # no list (n), fails too.
        greg = {**EN_AU_GREGORIAN, "f.Hm": "HH:nn"}
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        with self.assertRaisesRegex(SystemExit, "en-AU gregory"):
            self.lines(locales=d)

    def test_date_pattern_normalizes_cldr_typos(self):
        # CLDR 48.2 writes Y for y (gl's dateFormats full) and eee for EEE:
        # they are read as the letters meant. Quoted text is kept.
        d = dict(FIXTURE_DATES)
        greg = {
            **EN_AU_GREGORIAN,
            "d0": "EEEE, d 'de' MMMM 'de' Y",
            "f.yMMMd": "MMM d, YYYY 'Y'",
            "i.yMMMd.d": "eee d – eeeeee d, y",
        }
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        au = read_date_pair(self.parsed(locales=d), "en-AU", "gregory")
        self.assertEqual(au["d0"], "EEEE, d 'de' MMMM 'de' y")
        self.assertEqual(au["f.yMMMd"], "MMM d, yyyy 'Y'")
        self.assertEqual(au["i.yMMMd.d"], "EEE d – EEEEEE d, y")
        # e of any length is E (ko's "e요일" is "E요일").
        greg = {**EN_AU_GREGORIAN, "i.yMMMd.d": "e요일 d – ee d, y"}
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        au = read_date_pair(self.parsed(locales=d), "en-AU", "gregory")
        self.assertEqual(au["i.yMMMd.d"], "E요일 d – EE d, y")

    def test_alternates_dropped(self):
        lines = self.lines()
        p = parse_date_patterns(lines)
        en = read_date_pair(p, "en", "gregory")
        self.assertEqual(en, EN_GREGORIAN)
        for s in ("H:mm", "MM/dd/yy", "MMM d-d, y", "'week' W 'of' MMMM"):
            self.assertNotIn(s, lines)
        self.assertFalse([k for k in en if "-alt-" in k or "-count-" in k])

    def test_iso8601_from_root_xml(self):
        p = self.parsed()
        self.assertEqual(p["und"][1]["iso8601"], ISO8601_ENTRIES)
        self.assertNotIn("iso8601", p["en"][1])
        # Another locale on the chains with an iso8601 calendar fails, and
        # so does a root without one.
        en_xml = "<ldml><dates><calendars>" + ISO8601_XML + "</calendars></dates></ldml>"
        with self.assertRaisesRegex(SystemExit, "en_001.*iso8601"):
            self.lines(xml={"en_001": en_xml})
        # A locale of the file with iso8601 values of its own (zh's
        # intervals in CLDR 48.2) has them over root's, and so do its
        # children; a marked or provisional value is not one.
        child = (
            '<ldml><dates><calendars><calendar type="iso8601"><dateTimeFormats>'
            '<availableFormats><dateFormatItem id="Hm" draft="provisional">H.mm'
            "</dateFormatItem></availableFormats><intervalFormats>"
            '<intervalFormatItem id="yMMMd"><greatestDifference id="d" '
            'draft="contributed">y年MMMd–d日</greatestDifference>'
            '<greatestDifference id="M">\u2191\u2191\u2191</greatestDifference>'
            "</intervalFormatItem></intervalFormats></dateTimeFormats>"
            "</calendar></calendars></dates></ldml>"
        )
        p = parse_date_patterns(self.lines(xml={"en": child}))
        self.assertEqual(p["en"][1]["iso8601"], {"i.yMMMd.d": "y年MMMd–d日"})
        self.assertEqual(p["und"][1]["iso8601"], ISO8601_ENTRIES)
        want = {**ISO8601_ENTRIES, "i.yMMMd.d": "y年MMMd–d日"}
        self.assertEqual(read_date_pair(p, "en", "iso8601"), want)
        self.assertEqual(read_date_pair(p, "en-AU", "iso8601"), want)
        # One that only marks a value as inherited (bg's) defines nothing.
        marker = (
            '<ldml><dates><calendars><calendar type="iso8601"><dateTimeFormats>'
            '<appendItems><appendItem request="Timezone" draft="contributed">'
            "\u2191\u2191\u2191</appendItem></appendItems></dateTimeFormats>"
            "</calendar></calendars></dates></ldml>"
        )
        self.assertEqual(self.lines(xml={"en_001": marker}), self.lines())
        with self.assertRaisesRegex(SystemExit, "en_001.*iso8601"):
            self.lines(xml={"en_001": marker.replace("\u2191\u2191\u2191", "{1} {0}")})
        with self.assertRaisesRegex(SystemExit, "iso8601"):
            self.lines(xml={"root": "<ldml/>"})
        # An alias root's iso8601 has that the generator does not resolve.
        alias = ROOT_DATES_XML.replace(
            "<dateFormatItem id=\"Hm\">HH:mm</dateFormatItem>",
            "<alias source=\"locale\" path=\"../../calendar[@type='gregorian']\"/>",
        )
        with self.assertRaisesRegex(SystemExit, "alias"):
            self.lines(xml={"root": alias})

    def test_iso8601_at_time_alias_resolved_per_locale(self):
        # root's atTime full aliases its standard full format; a child that
        # overrides the standard one only gets it for atTime too, while
        # root's explicit atTime medium stays.
        child = (
            '<ldml><dates><calendars><calendar type="iso8601"><dateTimeFormats>'
            '<dateTimeFormatLength type="full"><dateTimeFormat><pattern>'
            "{1} 'um' {0}</pattern></dateTimeFormat></dateTimeFormatLength>"
            '<dateTimeFormatLength type="medium"><dateTimeFormat><pattern>'
            "{1}; {0}</pattern></dateTimeFormat></dateTimeFormatLength>"
            "</dateTimeFormats></calendar></calendars></dates></ldml>"
        )
        p = parse_date_patterns(self.lines(xml={"en": child}))
        en = read_date_pair(p, "en", "iso8601")
        self.assertEqual(en["c0"], "{1} 'um' {0}")
        self.assertEqual(en["a0"], "{1} 'um' {0}")
        self.assertEqual(en["c2"], "{1}; {0}")
        self.assertEqual(en["a2"], "{1} 'at' {0}")
        self.assertEqual(read_date_pair(p, "en-AU", "iso8601"), en)
        self.assertEqual(read_date_pair(p, "und", "iso8601"), ISO8601_ENTRIES)

    def test_date_patterns_reader(self):
        lines = ["strings 2", "y", "M", "locale und -", "gregory d0=1 f.y=0"]
        lines.append("generic = gregory")
        read = gen.date_patterns_reader(lines)
        self.assertEqual(read("und", "generic"), {"d0": "M", "f.y": "y"})
        # A line the reader cannot parse fails generation.
        for bad in (
            ["strings 1", "y", "locale und -", "gregory d0=1"],
            ["strings 1", "y", "locale und -", "gregory d0"],
            ["strings 1", "y", "locale und -", "gregory d0=0", "gregory t0=0"],
            ["strings 1", "y", "locale und -", "nocal d0=0"],
            ["strings 1", "y", "gregory d0=0"],
            ["strings x"],
        ):
            with self.assertRaises(SystemExit, msg=bad):
                gen.date_patterns_reader(bad)("und", "gregory")

    def test_rejects_tabs(self):
        d = dict(FIXTURE_DATES)
        greg = {**EN_AU_GREGORIAN, "d3": "d/M/yy\t"}
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        with self.assertRaisesRegex(SystemExit, "tab"):
            self.lines(locales=d)
        greg = {**EN_AU_GREGORIAN, "d3": "d/M/yy\n"}
        d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
        with self.assertRaisesRegex(SystemExit, "newline"):
            self.lines(locales=d)

    def test_date_pattern_placeholders(self):
        # The dateTimeFormats, the fallback, the append item and the leap
        # month need their placeholders.
        d = dict(FIXTURE_DATES)
        for key, bad in (
            ("c0", "{1} {1}"),
            ("a3", "{1}"),
            ("fb", "{0} -"),
            ("tz", "{0}"),
        ):
            greg = {**EN_AU_GREGORIAN, key: bad}
            d["en-AU"] = date_locale(greg, EN_AU_GENERIC, EN_AU_NAMES)
            with self.assertRaisesRegex(SystemExit, f"en-AU gregory {key}", msg=key):
                self.lines(locales=d)
        chinese = {**EN_AU_GENERIC, "lp": "bis"}
        d["en-AU"] = date_locale(
            EN_AU_GREGORIAN, EN_AU_GENERIC, EN_AU_NAMES, chinese=chinese
        )
        with self.assertRaisesRegex(SystemExit, "en-AU chinese lp"):
            self.lines(locales=d)


class DayPeriodsTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)

    def lines(self, locales=None, rules=None):
        tree = make_date_tree(
            tempfile.mkdtemp(dir=self._tmp.name),
            DAY_PERIOD_DATES if locales is None else locales,
            DATE_PARENTS,
            DATE_LIKELY,
            FIXTURE_RULES if rules is None else rules,
        )
        return gen.day_periods_lines(tree, DAY_PERIOD_TAGS)

    def test_day_period_rules(self):
        rules, _ = parse_day_periods(self.lines())
        self.assertEqual(
            rules["en"],
            {
                "midnight": (0, 0),
                "noon": (720, 720),
                "morning1": (0, 720),
                "afternoon1": (720, 1080),
                "evening1": (1080, 1260),
                "night1": (1260, 1440),
            },
        )
        # A period that crosses midnight keeps from > before.
        self.assertEqual(rules["bg"]["night1"], (1320, 240))
        # es_CO is written es-CO; both are there.
        self.assertEqual(rules["es"]["morning1"], (0, 360))
        self.assertEqual(rules["es-CO"]["morning2"], (0, 720))
        self.assertNotIn("morning1", rules["es-CO"])
        self.assertEqual(rules["und"], {"am": (0, 720), "pm": (720, 1440)})
        self.assertEqual(
            set(rules), {"und", "en", "bg", "es", "es-CO", "zh", "lij"}
        )
        bad = dict(FIXTURE_RULES, en={"noon": {"_at": "12:60"}})
        with self.assertRaisesRegex(SystemExit, "en noon"):
            self.lines(rules=bad)

    def test_day_period_rules_key_by_truncation(self):
        _, locales = parse_day_periods(self.lines())
        # zh-Hant's parent is und (CLDR's parentLocales), its rules zh's
        # (truncation, as ICU looks them up); zh-HK's (zh-Hant-HK) too.
        self.assertEqual(locales["zh-Hant"][:2], ("und", "zh"))
        self.assertEqual(locales["zh-HK"][:2], ("zh-Hant", "zh"))
        self.assertEqual(locales["es-CO"][:2], ("es", "es-CO"))
        self.assertEqual(locales["en-AU"][:2], ("en", "en"))
        self.assertEqual(locales["und"][:2], ("-", "und"))
        # zh-Hant lists names for zh's periods, which und does not have.
        self.assertEqual(locales["zh-Hant"][2]["morning1"], ("朝", "朝", "朝"))
        self.assertNotIn("morning1", locales["zh-HK"][2])
        self.assertEqual(
            day_period_names(locales, "zh-HK", "afternoon1"), ("夜", "夜", "夜")
        )

    def test_day_period_names(self):
        _, locales = parse_day_periods(self.lines())
        self.assertEqual(locales["en"][0], "und")
        en = locales["en"][2]
        self.assertEqual(en["morning1"], ("in the morning",) * 3)
        self.assertEqual(en["noon"], ("noon", "noon", "n"))
        # The widths are in the order abbreviated, wide, narrow.
        self.assertEqual(en["midnight"], ("midnight", "at midnight", "mi"))
        # am and pm are not en's periods.
        self.assertNotIn("am", en)
        self.assertEqual(locales["und"][2]["am"], ("AM", "AM", "AM"))
        # en-AU lists only the periods whose names differ from en's.
        self.assertEqual(
            locales["en-AU"][2],
            {
                "morning1": ("in the morning", "in the morning", "morning"),
                "noon": ("noon", "noon", "noon"),
            },
        )
        self.assertEqual(locales["es-CO"][2], {})
        # A period with no name in CLDR (lij's) has no line, and reads as "".
        self.assertEqual(locales["lij"][:3], ("und", "lij", {}))
        self.assertEqual(day_period_names(locales, "lij", "morning1"), ("", "", ""))
        # Every locale's periods read back to its names.
        for tag in DAY_PERIOD_TAGS:
            d = {"zh-HK": "zh-Hant-HK"}.get(tag, tag)
            names = DAY_PERIOD_DATES[d][1]
            key = locales[tag][1]
            for period in FIXTURE_RULES[key.replace("-", "_")]:
                want = tuple(
                    names[w].get(period, "") for w in ("abbreviated", "wide", "narrow")
                )
                self.assertEqual(day_period_names(locales, tag, period), want)

    def test_day_period_names_must_read_back(self):
        # en-AU lacks a name its parent has.
        d = dict(DAY_PERIOD_DATES)
        names = {w: dict(v) for w, v in EN_NAMES.items()}
        for w in names:
            del names[w]["noon"]
        d["en-AU"] = generic_locale(names)
        with self.assertRaisesRegex(SystemExit, "en-AU noon"):
            self.lines(locales=d)
        # A period named in some widths only.
        names = {w: dict(v) for w, v in EN_NAMES.items()}
        del names["narrow"]["noon"]
        d["en-AU"] = generic_locale(names)
        with self.assertRaisesRegex(SystemExit, "en-AU noon"):
            self.lines(locales=d)
        names = {w: dict(v) for w, v in EN_NAMES.items()}
        names["wide"]["noon"] = "no\ton"
        d["en-AU"] = generic_locale(names)
        with self.assertRaisesRegex(SystemExit, "tab"):
            self.lines(locales=d)


# The date fields of CLDR's dateFields.json that display-names.txt reads, by
# the draft's code.
DN_FIELDS = {
    "era": "era",
    "year": "year",
    "quarter": "quarter",
    "month": "month",
    "weekOfYear": "week",
    "weekday": "weekday",
    "day": "day",
    "dayPeriod": "dayperiod",
    "hour": "hour",
    "minute": "minute",
    "second": "second",
    "timeZoneName": "zone",
}
# CLDR's calendar names (localeDisplayNames.types.calendar keys) of the 16
# supported calendars, with their BCP 47 ids.
DN_CALENDARS = {
    "buddhist": "buddhist",
    "chinese": "chinese",
    "coptic": "coptic",
    "dangi": "dangi",
    "ethiopic-amete-alem": "ethioaa",
    "ethiopic": "ethiopic",
    "gregorian": "gregory",
    "hebrew": "hebrew",
    "indian": "indian",
    "islamic-civil": "islamic-civil",
    "islamic-tbla": "islamic-tbla",
    "islamic-umalqura": "islamic-umalqura",
    "iso8601": "iso8601",
    "japanese": "japanese",
    "persian": "persian",
    "roc": "roc",
}


def date_fields(names, **overrides):
    """A dateFields.json `fields` object: for each CLDR field of DN_FIELDS,
    `names(field)` as its long, short and narrow displayName, with
    `overrides` {"<field>[-short|-narrow]": name} over them, and fields and
    keys display-names.txt does not read."""
    out = {}
    for field in DN_FIELDS.values():
        for suffix, name in zip(("", "-short", "-narrow"), names(field)):
            out[field + suffix] = {
                "displayName": name,
                "relative-type-0": "this " + field,
            }
    for key, name in overrides.items():
        out[key] = dict(out.get(key, {}), displayName=name)
    out["dayperiod"]["displayName-alt-variant"] = "am/pm"
    out["dayOfYear"] = {"displayName": "day of year"}
    out["weekOfMonth"] = {"displayName": "week of month"}
    return out


DN_PATTERN = {
    "localePattern": "{0} ({1})",
    "localeSeparator": "{0}, {1}",
    "localeKeyTypePattern": "{0}: {1}",
}
EN_CALENDAR_NAMES = {
    **{name: name.title() + " Calendar" for name in DN_CALENDARS},
    "gregorian": "Gregorian Calendar",
    "islamic": "Hijri Calendar",
    "islamic-rgsa": "Hijri Calendar (Saudi Arabia, sighting)",
    "iso8601-alt-variant": "ISO 8601 Weeks",
    "core": "Minguo",
}

# und is CLDR's root, which has no names; en-AU's CLDR parent is en-001,
# which the file does not have, so its parent in the file is en.
FIXTURE_DISPLAY_NAMES = {
    "und": {
        "localeDisplayNames": {
            "localeDisplayPattern": DN_PATTERN,
            "codePatterns": {"language": "Language: {0}"},
        },
        "fields": date_fields(lambda f: (f.title(),) * 3),
    },
    "en": {
        "localeDisplayNames": {
            "localeDisplayPattern": DN_PATTERN,
            "types": {"calendar": EN_CALENDAR_NAMES},
        },
        "fields": date_fields(
            lambda f: (f, f[:2] + ".", f[:2]),
            **{"year-short": "yr.", "year-narrow": "yr", "weekday": "day of the week"},
        ),
        "languages": {
            "en": "English",
            "en-GB": "British English",
            "en-GB-alt-short": "UK English",
            "fon": "fon",
            "luo": "Luo",
            "nds-NL": "nds-NL",
            "yao-alt-menu": "yao",
        },
        "territories": {
            "US": "United States",
            "US-alt-short": "US",
            "UN-alt-variant": "UN",
            "GB": "United Kingdom",
            "GB-alt-short": "UK",
        },
        "scripts": {"Thai": "Thai", "Latn": "Latin", "Cham": "cham"},
        "variants": {"PINYIN": "PINYIN", "POSIX": "Computer"},
    },
    "en-AU": {
        "localeDisplayNames": {
            "localeDisplayPattern": DN_PATTERN,
            "types": {
                "calendar": dict(EN_CALENDAR_NAMES, roc="Republic of China Calendar")
            },
        },
        "fields": date_fields(
            lambda f: (f, f[:2] + ".", f[:2]),
            **{
                "year-short": "yr.",
                "year-narrow": "y",
                "weekday": "day of the week",
            },
        ),
        "languages": {"en": "English", "fon": "fon", "nds-NL": "nds-NL"},
        "territories": {"US": "United States", "US-alt-short": "US"},
        "scripts": {"Thai": "Thai", "Latn": "Latin"},
        "variants": {"PINYIN": "PINYIN"},
    },
}


def make_display_names_tree(root, locales):
    """Write fixture cldr-localenames-full, cldr-dates-full and cldr-core
    files under `root` for `locales` ({dir: {"localeDisplayNames": ...,
    "fields": ..., "languages": ...}}), and return a tree reading them."""
    for loc, data in locales.items():
        ldn = ["cldr-localenames-full", "package", "main", loc]
        write_json(
            root,
            ldn + ["localeDisplayNames.json"],
            {"main": {loc: {"localeDisplayNames": data["localeDisplayNames"]}}},
        )
        for kind in ("languages", "territories", "scripts", "variants"):
            if kind in data:
                write_json(
                    root,
                    ldn + [f"{kind}.json"],
                    {"main": {loc: {"localeDisplayNames": {kind: data[kind]}}}},
                )
        write_json(
            root,
            ["cldr-dates-full", "package", "main", loc, "dateFields.json"],
            {"main": {loc: {"dates": {"fields": data["fields"]}}}},
        )
    write_json(
        root,
        ["cldr-core", "package", "supplemental", "parentLocales.json"],
        {"supplemental": {"parentLocales": {"parentLocale": FIXTURE_PARENTS}}},
    )
    write_json(
        root,
        ["cldr-core", "package", "supplemental", "likelySubtags.json"],
        {"supplemental": {"likelySubtags": FIXTURE_LIKELY}},
    )
    return gen.Tree(root)


def parse_display_names(lines):
    """Parse display-names.txt lines: {tag: (parent, {key: value})}, the keys
    in the file's order."""
    out = {}
    cur = None
    for line in lines:
        if line.startswith("#"):
            continue
        if "\t" not in line:
            f = line.split(" ")
            assert len(f) == 3 and f[0] == "locale", line
            cur = out[f[1]] = (f[2], {})
            continue
        key, value = line.split("\t")
        assert key not in cur[1], line
        cur[1][key] = value
    return out


def read_display_name(parsed, tag, key):
    """The value of `key` for `tag`: its own line, else its parent's, up to
    und; None if none has one."""
    while tag != "-":
        parent, entries = parsed[tag]
        if key in entries:
            return entries[key]
        tag = parent
    return None


class DisplayNamesTest(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)

    def lines(self, locales=None, tags=("en", "en-AU")):
        tree = make_display_names_tree(
            tempfile.mkdtemp(dir=self._tmp.name),
            FIXTURE_DISPLAY_NAMES if locales is None else locales,
        )
        return gen.display_names_lines(tree, list(tags))

    def parsed(self, **kw):
        return parse_display_names(self.lines(**kw))

    def fixture(self, tag, **changes):
        """FIXTURE_DISPLAY_NAMES with `tag`'s data changed by `changes`."""
        d = dict(FIXTURE_DISPLAY_NAMES)
        d[tag] = dict(d[tag], **changes)
        return d

    def test_display_names_calendars(self):
        p = self.parsed()
        en = p["en"][1]
        self.assertEqual(en["ca.gregory"], "Gregorian Calendar")
        self.assertEqual(en["ca.ethioaa"], "Ethiopic-Amete-Alem Calendar")
        self.assertEqual(en["ca.islamic-civil"], "Islamic-Civil Calendar")
        self.assertEqual(
            sorted(k for k in en if k.startswith("ca.")),
            sorted("ca." + c for c in DN_CALENDARS.values()),
        )
        for key in ("ca.gregorian", "ca.ethiopic-amete-alem", "ca.islamic"):
            self.assertNotIn(key, en)
        for key in ("ca.islamic-rgsa", "ca.iso8601-alt-variant", "ca.core"):
            self.assertNotIn(key, en)
        # Root has no calendar names, so und has no ca line.
        self.assertFalse([k for k in p["und"][1] if k.startswith("ca.")])

    def test_display_names_fields(self):
        # displayName-alt-variant (en's dayperiod "am/pm") is not written.
        self.assertNotIn("am/pm", "\n".join(self.lines()))
        p = self.parsed()
        self.assertEqual(read_display_name(p, "en", "f.year.long"), "year")
        self.assertEqual(read_display_name(p, "en", "f.year.short"), "yr.")
        self.assertEqual(read_display_name(p, "en", "f.year.narrow"), "yr")
        self.assertEqual(read_display_name(p, "en", "f.weekOfYear.long"), "week")
        self.assertEqual(read_display_name(p, "en", "f.weekOfYear.short"), "we.")
        self.assertEqual(read_display_name(p, "en", "f.dayPeriod.long"), "dayperiod")
        self.assertEqual(read_display_name(p, "en", "f.dayPeriod.narrow"), "da")
        self.assertEqual(read_display_name(p, "en", "f.timeZoneName.long"), "zone")
        self.assertEqual(read_display_name(p, "und", "f.timeZoneName.short"), "Zone")
        self.assertEqual(
            read_display_name(p, "en", "f.weekday.long"), "day of the week"
        )
        # Exactly the 12 codes in three styles; CLDR's own field names and
        # the other fields are not written.
        keys = {k for _, (_, e) in p.items() for k in e if k.startswith("f.")}
        self.assertEqual(
            keys,
            {f"f.{c}.{s}" for c in DN_FIELDS for s in ("long", "short", "narrow")},
        )

    def test_display_names_patterns(self):
        p = self.parsed()
        self.assertEqual(p["und"][1]["lp"], "{0} ({1})")
        self.assertEqual(p["und"][1]["ls"], "{0}, {1}")
        # en's are und's, so it has no line of its own.
        self.assertNotIn("lp", p["en"][1])
        self.assertEqual(read_display_name(p, "en-AU", "ls"), "{0}, {1}")
        pattern = dict(DN_PATTERN, localePattern="{0}（{1}）")
        d = self.fixture(
            "en",
            localeDisplayNames=dict(
                FIXTURE_DISPLAY_NAMES["en"]["localeDisplayNames"],
                localeDisplayPattern=pattern,
            ),
        )
        # en-AU's pattern is und's, which differs from en's, its parent's:
        # it is written.
        p = parse_display_names(self.lines(locales=d))
        self.assertEqual(p["en"][1]["lp"], "{0}（{1}）")
        self.assertEqual(p["en-AU"][1]["lp"], "{0} ({1})")
        self.assertNotIn("ls", p["en-AU"][1])
        # A pattern without both placeholders fails.
        pattern = dict(DN_PATTERN, localeSeparator="{0},")
        d = self.fixture(
            "en",
            localeDisplayNames=dict(
                FIXTURE_DISPLAY_NAMES["en"]["localeDisplayNames"],
                localeDisplayPattern=pattern,
            ),
        )
        with self.assertRaisesRegex(SystemExit, r"display names en ls.*\{1\}"):
            self.lines(locales=d)

    def test_display_names_code_equal(self):
        p = self.parsed()
        en = p["en"][1]
        eq = {k: v for k, v in en.items() if k.startswith("eq.")}
        self.assertEqual(
            eq,
            {
                "eq.dialect.long.nds-NL": "",
                "eq.language.long.fon": "",
                "eq.region.short.US": "",
                "eq.script.long.Thai": "",
                "eq.variant.long.PINYIN": "",
            },
        )
        # A name that differs from its code ("Luo", "cham", "UK") writes
        # nothing, and so does an alternate other than short (UN-alt-variant,
        # yao-alt-menu).
        # en-AU's are en's, so it has none of its own.
        self.assertFalse([k for k in p["en-AU"][1] if k.startswith("eq.")])
        self.assertIn("eq.region.short.US\t", self.lines())

    def test_display_names_dedup(self):
        p = self.parsed()
        self.assertEqual(p["und"][0], "-")
        self.assertEqual(p["en"][0], "und")
        self.assertEqual(p["en-AU"][0], "en")
        # en-AU differs from en in roc's name and year's narrow name only.
        self.assertEqual(
            p["en-AU"][1],
            {"ca.roc": "Republic of China Calendar", "f.year.narrow": "y"},
        )
        # en lists only what differs from und: und's field names are
        # capitalized, so every field name is en's own.
        self.assertEqual(len([k for k in p["en"][1] if k.startswith("f.")]), 36)
        # und has every key it defines in full.
        self.assertEqual(len(p["und"][1]), 38)
        # A child that lacks a name its parent chain gives fails.
        names = dict(FIXTURE_DISPLAY_NAMES["en-AU"]["localeDisplayNames"])
        cal = dict(names["types"]["calendar"])
        del cal["roc"]
        names["types"] = {"calendar": cal}
        with self.assertRaisesRegex(SystemExit, "display names en-AU.*ca.roc"):
            self.lines(locales=self.fixture("en-AU", localeDisplayNames=names))
        # A child with its own name for a code its parent's name equals
        # (CLDR 48.2's nn "tivi", hi-Latn short "America") inherits the eq
        # entry, which applies only where ICU4X has no name; ICU4X has the
        # child's.
        territories = {"US": "United States", "US-alt-short": "USA"}
        p = parse_display_names(
            self.lines(locales=self.fixture("en-AU", territories=territories))
        )
        self.assertNotIn("eq.region.short.US", p["en-AU"][1])
        self.assertEqual(read_display_name(p, "en-AU", "eq.region.short.US"), "")
        # A child with no name for it at all fails, and so does one whose
        # other style only has a name.
        for territories in (
            {"US": "United States"},
            {"US": "United States", "US-alt-variant": "USA"},
        ):
            d = self.fixture("en-AU", territories=territories)
            with self.assertRaisesRegex(
                SystemExit, "display names en-AU.*eq.region.short.US"
            ):
                self.lines(locales=d)

    def test_display_names_checks(self):
        fields = dict(FIXTURE_DISPLAY_NAMES["und"]["fields"])
        del fields["era-narrow"]
        with self.assertRaisesRegex(SystemExit, "display names und.*f.era.narrow"):
            self.lines(locales=self.fixture("und", fields=fields))
        names = dict(FIXTURE_DISPLAY_NAMES["und"]["localeDisplayNames"])
        del names["localeDisplayPattern"]
        with self.assertRaisesRegex(SystemExit, "display names und.*lp"):
            self.lines(locales=self.fixture("und", localeDisplayNames=names))
        # en (and so en-AU) lacks a supported calendar.
        d = dict(FIXTURE_DISPLAY_NAMES)
        for tag in ("en", "en-AU"):
            names = dict(d[tag]["localeDisplayNames"])
            cal = dict(names["types"]["calendar"])
            del cal["ethiopic-amete-alem"]
            names["types"] = {"calendar": cal}
            d[tag] = dict(d[tag], localeDisplayNames=names)
        with self.assertRaisesRegex(SystemExit, "display names en.*ca.ethioaa"):
            self.lines(locales=d)
        # Tabs and newlines fail, and so does an empty name.
        fields = date_fields(lambda f: (f, f[:2] + ".", f[:2]), year="ye\tar")
        with self.assertRaisesRegex(SystemExit, "tab"):
            self.lines(locales=self.fixture("en", fields=fields))
        fields = date_fields(lambda f: (f, f[:2] + ".", f[:2]), year="")
        with self.assertRaisesRegex(SystemExit, "display names en f.year.long"):
            self.lines(locales=self.fixture("en", fields=fields))


if __name__ == "__main__":
    unittest.main()

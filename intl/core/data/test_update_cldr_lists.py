#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Unit tests of update-cldr-lists.py's extraction of the -u- and -t- value
aliases (on fixture bcp47 XML), of number-symbols.txt, units.txt and
currency-symbols.txt (on fixture CLDR JSON), of explicit-number-systems.txt
(on fixture common/main XML) and of compact-currency.txt (on both). Run by
check-intl-data; no network."""

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
                "ar\tarab\t%\tE\t∞\tNaN\t~\t#,##0‏%\t{0}–{1}\t\t",
                "ar\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t",
                "en\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t",
                "und\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t",
            ],
        )

    def test_currency_separators(self):
        n = dict(FIXTURE_NUMBERS)
        n["en"] = numbers(("latn", ".", "#,##0%"))
        n["en"]["symbols-numberSystem-latn"]["currencyGroup"] = "."
        n["en"]["symbols-numberSystem-latn"]["currencyDecimal"] = "\u00b7"
        lines = gen.number_symbols_lines(self.tree(n), ["en"])
        self.assertIn("en\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\u00b7\t.", lines)

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
        self.assertIn("zh-HK\tlatn\t%\tE\t∞\tNaN\t~\t#,##0%\t{0}–{1}\t\t", lines)
        # By the script of its language, when the tag has no entry.
        self.assertIn("zh-SG\tlatn\t%\tE\t∞\tNaN\t~\t#,##0 %\t{0}–{1}\t\t", lines)
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
                    + ["∞", "NaN", "~"]
                ),
                "percent\tarab\t#,##0%",
                "currency\tarab\t#,##0.00\u00a0¤\t\t=standard\t",
                "locale\tar\tund",
                "\t".join(["symbols", "arabext", e, e, e, e, e, e, e, "ليس رقمًا", e]),
                "locale\ten\tund",
                "\t".join(["symbols", "arab", e, e, e, "\u200f+", e, e, e, e, e]),
                "\t".join(["symbols", "bali", e, e, e, e, e, e, e, "nb", e]),
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


if __name__ == "__main__":
    unittest.main()

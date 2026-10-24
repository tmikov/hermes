#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Unit tests of update-cldr-lists.py's extraction of the -u- and -t- value
aliases, on fixture bcp47 XML. Run by check-intl-data; no network."""

import importlib.util
import os
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


if __name__ == "__main__":
    unittest.main()

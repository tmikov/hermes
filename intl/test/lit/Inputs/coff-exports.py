# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""Print the exports of a PE (Windows) DLL, one per line: the name, or
#<ordinal> for an export without one. Used by exports.js for the plugin DLL,
so that the check needs no Windows SDK or LLVM tool. Exits 1 with a message
on stderr if the file is not a PE image."""

import struct
import sys


def fail(message):
    sys.stderr.write("coff-exports.py: " + message + "\n")
    sys.exit(1)


def exports(data):
    """The export names (or "#<ordinal>") of the PE image in data."""

    def u16(off):
        return struct.unpack_from("<H", data, off)[0]

    def u32(off):
        return struct.unpack_from("<I", data, off)[0]

    if data[:2] != b"MZ":
        fail("not a PE image (no MZ header)")
    pe = u32(0x3C)
    if data[pe : pe + 4] != b"PE\0\0":
        fail("not a PE image (no PE signature)")
    num_sections = u16(pe + 6)
    opt_size = u16(pe + 20)
    opt = pe + 24
    magic = u16(opt)
    if magic == 0x10B:
        dirs = opt + 96
    elif magic == 0x20B:
        dirs = opt + 112
    else:
        fail("unknown optional header magic 0x%x" % magic)
    if u32(dirs - 4) < 1:
        return []
    export_rva = u32(dirs)
    if export_rva == 0:
        return []

    sections = []
    for i in range(num_sections):
        s = opt + opt_size + 40 * i
        vsize, va, raw_size, raw_ptr = struct.unpack_from("<IIII", data, s + 8)
        sections.append((va, max(vsize, raw_size), raw_ptr))

    def offset(rva):
        for va, size, raw_ptr in sections:
            if va <= rva < va + size:
                return rva - va + raw_ptr
        fail("RVA 0x%x is in no section" % rva)

    def cstring(rva):
        start = offset(rva)
        return data[start : data.index(b"\0", start)].decode("utf-8")

    d = offset(export_rva)
    base, num_functions, num_names, functions, names, ordinals = struct.unpack_from(
        "<IIIIII", data, d + 16
    )
    named = {}
    for i in range(num_names):
        index = u16(offset(ordinals) + 2 * i)
        named[index] = cstring(u32(offset(names) + 4 * i))
    result = []
    for index in range(num_functions):
        if u32(offset(functions) + 4 * index) == 0:
            continue
        result.append(named.get(index, "#%d" % (base + index)))
    return result


def main():
    if len(sys.argv) != 2:
        fail("usage: coff-exports.py <dll>")
    with open(sys.argv[1], "rb") as f:
        data = f.read()
    for name in exports(data):
        print(name)


if __name__ == "__main__":
    main()

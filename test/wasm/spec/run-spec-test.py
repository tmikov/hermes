#!/usr/bin/env python3
# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is licensed under the MIT license found in the
# LICENSE file in the root directory of this source tree.

"""
Wasm spec test runner for Hermes.

Converts a .wast file to JSON + .wasm modules via wast2json, generates a JS
test harness, and runs it with hermes to verify assertions.

Usage:
    python3 run-spec-test.py --wast2json PATH --hermes PATH test.wast

Exit code 0 = all assertions passed, non-zero = failures.
"""

import argparse
import json
import os
import struct
import subprocess
import sys
import tempfile


def f32_bits_to_js(bits_str):
    """Convert f32 bit pattern (as decimal string) to JS number literal."""
    bits = int(bits_str) & 0xFFFFFFFF
    val = struct.unpack('<f', struct.pack('<I', bits))[0]
    # Check for special values
    if bits == 0x7F800000:
        return "Infinity"
    if bits == 0xFF800000:
        return "-Infinity"
    if bits == 0x80000000:
        return "-0.0"
    if bits == 0x00000000:
        return "0.0"
    # NaN
    if (bits & 0x7F800000) == 0x7F800000 and (bits & 0x007FFFFF) != 0:
        return "NaN"
    return repr(val)


def f64_bits_to_js(bits_str):
    """Convert f64 bit pattern (as decimal string) to JS number literal."""
    bits = int(bits_str) & 0xFFFFFFFFFFFFFFFF
    val = struct.unpack('<d', struct.pack('<Q', bits))[0]
    if bits == 0x7FF0000000000000:
        return "Infinity"
    if bits == 0xFFF0000000000000:
        return "-Infinity"
    if bits == 0x8000000000000000:
        return "-0.0"
    if bits == 0x0000000000000000:
        return "0.0"
    # NaN
    if (bits & 0x7FF0000000000000) == 0x7FF0000000000000 and \
       (bits & 0x000FFFFFFFFFFFFF) != 0:
        return "NaN"
    return repr(val)


def i32_value_to_js(val_str):
    """Convert i32 value (unsigned decimal string) to JS signed int32."""
    val = int(val_str) & 0xFFFFFFFF
    if val >= 0x80000000:
        val -= 0x100000000
    return str(val)


def i64_value_to_bigint_literal(val_str):
    """Convert i64 value (unsigned decimal string) to JS BigInt literal."""
    bits = int(val_str) & 0xFFFFFFFFFFFFFFFF
    # Convert to signed for BigInt constructor
    if bits >= 0x8000000000000000:
        signed_val = bits - 0x10000000000000000
    else:
        signed_val = bits
    return f'BigInt("{signed_val}")'


def value_to_js_arg(val):
    """Convert a wast2json value to a JS argument expression."""
    typ = val['type']
    v = val['value']
    if typ == 'i32':
        return i32_value_to_js(v)
    elif typ == 'i64':
        return i64_value_to_bigint_literal(v)
    elif typ == 'f32':
        return f32_bits_to_js(v)
    elif typ == 'f64':
        return f64_bits_to_js(v)
    else:
        return "undefined"


def escape_js_string(s):
    """Escape a string for use inside a JS double-quoted string literal."""
    s = s.replace('\\', '\\\\')
    s = s.replace('"', '\\"')
    s = s.replace('\n', '\\n')
    s = s.replace('\r', '\\r')
    s = s.replace('\t', '\\t')
    # Escape any other non-printable or non-ASCII chars as unicode escapes.
    result = []
    for ch in s:
        code = ord(ch)
        if code < 0x20 or code > 0x7e:
            if code > 0xffff:
                result.append(f'\\u{{{code:x}}}')
            else:
                result.append(f'\\u{code:04x}')
        else:
            result.append(ch)
    return ''.join(result)


def gen_expected_check(expected, result_var):
    """Generate JS code to check expected value(s) against a result.

    For single-value returns, result_var is the value directly.
    For multi-value returns (len(expected) > 1), result_var is an Array.
    """
    if not expected:
        return f"true /* void */"

    if len(expected) == 1:
        return gen_single_expected_check(expected[0], result_var)

    # Multi-value: result_var is an Array. Check each element.
    checks = []
    for i, val in enumerate(expected):
        check = gen_single_expected_check(val, f"{result_var}[{i}]")
        checks.append(check)
    return " && ".join(checks)


def gen_single_expected_check(val, result_expr):
    """Generate JS code to check a single expected value."""
    typ = val['type']
    v = val.get('value', None)

    if v is None:
        return "true"

    if v == "nan:canonical" or v == "nan:arithmetic":
        return f"Number.isNaN({result_expr})"

    if typ == 'i32':
        expected_js = i32_value_to_js(v)
        return f"(({result_expr} | 0) === ({expected_js} | 0))"
    elif typ == 'i64':
        expected_js = i64_value_to_bigint_literal(v)
        return f"({result_expr} === {expected_js})"
    elif typ == 'f32':
        bits = int(v) & 0xFFFFFFFF
        if (bits & 0x7F800000) == 0x7F800000 and (bits & 0x007FFFFF) != 0:
            return f"Number.isNaN({result_expr})"
        js_val = f32_bits_to_js(v)
        if js_val == "-0.0":
            return f"(Object.is({result_expr}, -0))"
        if js_val == "0.0":
            return f"({result_expr} === 0 && !Object.is({result_expr}, -0))"
        return f"({result_expr} === {js_val})"
    elif typ == 'f64':
        bits = int(v) & 0xFFFFFFFFFFFFFFFF
        if (bits & 0x7FF0000000000000) == 0x7FF0000000000000 and \
           (bits & 0x000FFFFFFFFFFFFF) != 0:
            return f"Number.isNaN({result_expr})"
        js_val = f64_bits_to_js(v)
        if js_val == "-0.0":
            return f"(Object.is({result_expr}, -0))"
        if js_val == "0.0":
            return f"({result_expr} === 0 && !Object.is({result_expr}, -0))"
        return f"({result_expr} === {js_val})"
    else:
        return "true"


def generate_js_harness(spec, wasm_dir):
    """Generate JS test harness code from the spec JSON."""
    lines = []
    lines.append("// Auto-generated Wasm spec test harness")
    lines.append("'use strict';")
    lines.append("")
    lines.append("var passed = 0;")
    lines.append("var failed = 0;")
    lines.append("var skipped = 0;")
    lines.append("")
    lines.append("// Module registry for 'register' commands")
    lines.append("var registry = {};")
    lines.append("")
    lines.append("// Current module instance and its exports")
    lines.append("var currentInstance = null;")
    lines.append("var currentExports = null;")
    lines.append("")
    lines.append("// Named module instances")
    lines.append("var namedModules = {};")
    lines.append("")

    # Helper function to load and instantiate a module
    lines.append("function loadModule(wasmPath, importObj) {")
    lines.append("  var bytes = hermescli.loadFile(wasmPath);")
    lines.append("  var mod = new WebAssembly.Module(bytes);")
    lines.append("  var inst = new WebAssembly.Instance(mod, importObj || {});")
    lines.append("  return inst;")
    lines.append("}")
    lines.append("")

    # Helper to build import object from registry
    lines.append("// Standard spectest module per the Wasm spec")
    lines.append("var spectest = {")
    lines.append("  print_i32: function() {},")
    lines.append("  print_i64: function() {},")
    lines.append("  print_f32: function() {},")
    lines.append("  print_f64: function() {},")
    lines.append("  print_i32_f32: function() {},")
    lines.append("  print_f64_f64: function() {},")
    lines.append("  print: function() {},")
    lines.append("  global_i32: new WebAssembly.Global({value: 'i32', mutable: false}, 666),")
    # An i64 global takes a BigInt: a Number cannot represent every i64
    # exactly, and Global.prototype.value is a BigInt for i64 per spec.
    lines.append("  global_i64: new WebAssembly.Global({value: 'i64', mutable: false}, 666n),")
    lines.append("  global_f32: new WebAssembly.Global({value: 'f32', mutable: false}, 666.6),")
    lines.append("  global_f64: new WebAssembly.Global({value: 'f64', mutable: false}, 666.6),")
    lines.append("  table: new WebAssembly.Table({element: 'anyfunc', initial: 10, maximum: 20}),")
    lines.append("  memory: new WebAssembly.Memory({initial: 1, maximum: 2}),")
    lines.append("};")
    lines.append("registry['spectest'] = spectest;")
    lines.append("")
    lines.append("function buildImports(mod) {")
    lines.append("  var imports = WebAssembly.Module.imports(mod);")
    lines.append("  var importObj = {};")
    lines.append("  for (var i = 0; i < imports.length; i++) {")
    lines.append("    var imp = imports[i];")
    lines.append("    var modName = imp.module;")
    lines.append("    if (!importObj[modName]) importObj[modName] = {};")
    lines.append("    if (registry[modName] && registry[modName][imp.name] !== undefined) {")
    lines.append("      importObj[modName][imp.name] = registry[modName][imp.name];")
    lines.append("    }")
    lines.append("  }")
    lines.append("  return importObj;")
    lines.append("}")
    lines.append("")

    # Helper to load module with auto-resolved imports
    lines.append("function loadModuleWithImports(wasmPath) {")
    lines.append("  var bytes = hermescli.loadFile(wasmPath);")
    lines.append("  var mod = new WebAssembly.Module(bytes);")
    lines.append("  var importObj = buildImports(mod);")
    lines.append("  var inst = new WebAssembly.Instance(mod, importObj);")
    lines.append("  return inst;")
    lines.append("}")
    lines.append("")

    # Process each command
    for i, cmd in enumerate(spec['commands']):
        cmd_type = cmd['type']
        line = cmd.get('line', '?')

        if cmd_type == 'module':
            wasm_file = os.path.join(wasm_dir, cmd['filename'])
            name = cmd.get('name', None)
            lines.append(f"// Line {line}: module {cmd['filename']}")
            lines.append("try {")
            lines.append(f'  currentInstance = loadModuleWithImports("{wasm_file}");')
            lines.append("  currentExports = currentInstance.exports;")
            if name:
                lines.append(f'  namedModules["{escape_js_string(name)}"] = currentInstance;')
            lines.append("} catch(e) {")
            lines.append(f'  print("FAIL: line {line}: module load failed: " + e.message);')
            lines.append("  failed++;")
            lines.append("  currentInstance = null;")
            lines.append("  currentExports = null;")
            lines.append("}")
            lines.append("")

        elif cmd_type == 'register':
            as_name = cmd['as']
            name = cmd.get('name', None)
            lines.append(f"// Line {line}: register as '{as_name}'")
            if name:
                esc_name = escape_js_string(name)
                esc_as = escape_js_string(as_name)
                lines.append(f'if (namedModules["{esc_name}"]) registry["{esc_as}"] = namedModules["{esc_name}"].exports;')
            else:
                lines.append(f'if (currentExports) registry["{escape_js_string(as_name)}"] = currentExports;')
            lines.append("")

        elif cmd_type == 'assert_return':
            action = cmd['action']
            expected = cmd.get('expected', [])

            if action['type'] == 'invoke':
                field = action['field']
                esc_field = escape_js_string(field)
                args = action.get('args', [])
                module_name = action.get('module', None)

                js_args = ", ".join(value_to_js_arg(a) for a in args)

                lines.append(f"// Line {line}: assert_return invoke {esc_field}")
                lines.append("try {")
                if module_name:
                    lines.append(f'  var exports__ = namedModules["{escape_js_string(module_name)}"].exports;')
                else:
                    lines.append("  var exports__ = currentExports;")
                lines.append(f'  var result__ = exports__["{esc_field}"]({js_args});')

                if expected:
                    check = gen_expected_check(expected, "result__")
                    lines.append(f"  if ({check}) {{")
                    lines.append("    passed++;")
                    lines.append("  } else {")
                    lines.append(f'    print("FAIL: line {line}: {esc_field}: expected " + {json.dumps(str(expected))} + " got " + result__);')
                    lines.append("    failed++;")
                    lines.append("  }")
                else:
                    # void return — just check it didn't throw
                    lines.append("  passed++;")

                lines.append("} catch(e) {")
                lines.append(f'  print("FAIL: line {line}: {esc_field}: unexpected exception: " + e.message);')
                lines.append("  failed++;")
                lines.append("}")
                lines.append("")

            elif action['type'] == 'get':
                field = action['field']
                esc_field = escape_js_string(field)
                module_name = action.get('module', None)

                lines.append(f"// Line {line}: assert_return get {esc_field}")
                lines.append("try {")
                if module_name:
                    lines.append(f'  var exports__ = namedModules["{escape_js_string(module_name)}"].exports;')
                else:
                    lines.append("  var exports__ = currentExports;")
                lines.append(f'  var result__ = exports__["{esc_field}"];')

                # For global exports, the value might be a WebAssembly.Global
                # Need to handle both raw value and Global.value
                lines.append("  if (typeof result__ === 'object' && result__ !== null && result__.valueOf) {")
                lines.append("    result__ = result__.valueOf();")
                lines.append("  }")

                if expected:
                    check = gen_expected_check(expected, "result__")
                    lines.append(f"  if ({check}) {{")
                    lines.append("    passed++;")
                    lines.append("  } else {")
                    lines.append(f'    print("FAIL: line {line}: get {esc_field}: expected " + {json.dumps(str(expected))} + " got " + result__);')
                    lines.append("    failed++;")
                    lines.append("  }")
                else:
                    lines.append("  passed++;")

                lines.append("} catch(e) {")
                lines.append(f'  print("FAIL: line {line}: get {esc_field}: unexpected exception: " + e.message);')
                lines.append("  failed++;")
                lines.append("}")
                lines.append("")

        elif cmd_type == 'assert_trap':
            action = cmd['action']
            text = cmd.get('text', '')

            if action['type'] == 'invoke':
                field = action['field']
                esc_field = escape_js_string(field)
                args = action.get('args', [])
                module_name = action.get('module', None)
                js_args = ", ".join(value_to_js_arg(a) for a in args)

                lines.append(f"// Line {line}: assert_trap invoke {esc_field}")
                lines.append("try {")
                if module_name:
                    lines.append(f'  var exports__ = namedModules["{escape_js_string(module_name)}"].exports;')
                else:
                    lines.append("  var exports__ = currentExports;")
                lines.append(f'  exports__["{esc_field}"]({js_args});')
                lines.append(f'  print("FAIL: line {line}: {esc_field}: expected trap but succeeded");')
                lines.append("  failed++;")
                lines.append("} catch(e) {")
                lines.append("  passed++;")
                lines.append("}")
                lines.append("")

        elif cmd_type == 'assert_invalid':
            wasm_file = os.path.join(wasm_dir, cmd['filename'])
            text = cmd.get('text', '')
            lines.append(f"// Line {line}: assert_invalid")
            lines.append("try {")
            lines.append(f'  var bytes__ = hermescli.loadFile("{wasm_file}");')
            lines.append(f"  var valid__ = WebAssembly.validate(bytes__);")
            lines.append("  if (!valid__) {")
            lines.append("    passed++;")
            lines.append("  } else {")
            lines.append("    // Our validator didn't catch it — count as failure.")
            lines.append("    // Don't try to compile: invalid modules may crash the compiler.")
            lines.append(f'    print("FAIL: line {line}: assert_invalid: validate returned true");')
            lines.append("    failed++;")
            lines.append("  }")
            lines.append("} catch(e) {")
            lines.append("  // File load or compilation failed — that's valid for assert_invalid")
            lines.append("  passed++;")
            lines.append("}")
            lines.append("")

        elif cmd_type == 'assert_malformed':
            wasm_file = os.path.join(wasm_dir, cmd['filename'])
            module_type = cmd.get('module_type', 'binary')
            lines.append(f"// Line {line}: assert_malformed")
            if module_type == 'text':
                # Text-format malformed tests — we can't parse WAT, skip
                lines.append("skipped++;")
            else:
                lines.append("try {")
                lines.append(f'  var bytes__ = hermescli.loadFile("{wasm_file}");')
                lines.append("  new WebAssembly.Module(bytes__);")
                lines.append(f'  print("FAIL: line {line}: assert_malformed: module compiled successfully");')
                lines.append("  failed++;")
                lines.append("} catch(e) {")
                lines.append("  passed++;")
                lines.append("}")
            lines.append("")

        elif cmd_type == 'assert_unlinkable':
            wasm_file = os.path.join(wasm_dir, cmd['filename'])
            lines.append(f"// Line {line}: assert_unlinkable")
            lines.append("try {")
            lines.append(f'  var bytes__ = hermescli.loadFile("{wasm_file}");')
            lines.append("  var mod__ = new WebAssembly.Module(bytes__);")
            lines.append("  var importObj__ = buildImports(mod__);")
            lines.append("  new WebAssembly.Instance(mod__, importObj__);")
            lines.append(f'  print("FAIL: line {line}: assert_unlinkable: instantiation succeeded");')
            lines.append("  failed++;")
            lines.append("} catch(e) {")
            lines.append("  passed++;")
            lines.append("}")
            lines.append("")

        elif cmd_type == 'assert_uninstantiable':
            wasm_file = os.path.join(wasm_dir, cmd['filename'])
            lines.append(f"// Line {line}: assert_uninstantiable")
            lines.append("try {")
            lines.append(f'  var bytes__ = hermescli.loadFile("{wasm_file}");')
            lines.append("  var mod__ = new WebAssembly.Module(bytes__);")
            lines.append("  var importObj__ = buildImports(mod__);")
            lines.append("  new WebAssembly.Instance(mod__, importObj__);")
            lines.append(f'  print("FAIL: line {line}: assert_uninstantiable: instantiation succeeded");')
            lines.append("  failed++;")
            lines.append("} catch(e) {")
            lines.append("  passed++;")
            lines.append("}")
            lines.append("")

        elif cmd_type == 'action':
            action = cmd['action']
            if action['type'] == 'invoke':
                field = action['field']
                esc_field = escape_js_string(field)
                args = action.get('args', [])
                module_name = action.get('module', None)
                js_args = ", ".join(value_to_js_arg(a) for a in args)

                lines.append(f"// Line {line}: action invoke {esc_field}")
                lines.append("try {")
                if module_name:
                    lines.append(f'  namedModules["{escape_js_string(module_name)}"].exports["{esc_field}"]({js_args});')
                else:
                    lines.append(f'  currentExports["{esc_field}"]({js_args});')
                lines.append("} catch(e) {}")
                lines.append("")

    # Print summary
    lines.append('print("PASSED: " + passed);')
    lines.append('print("FAILED: " + failed);')
    lines.append('print("SKIPPED: " + skipped);')
    lines.append("if (failed > 0) {")
    lines.append('  print("SPEC TEST FAILED");')
    lines.append("} else {")
    lines.append('  print("SPEC TEST PASSED");')
    lines.append("}")

    return "\n".join(lines) + "\n"


def main():
    parser = argparse.ArgumentParser(description='Run Wasm spec tests')
    parser.add_argument('wast_file', help='Path to .wast file')
    parser.add_argument('--wast2json', required=True, help='Path to wast2json binary')
    parser.add_argument('--hermes', required=True, help='Path to hermes binary')
    parser.add_argument('--verbose', '-v', action='store_true',
                        help='Print verbose output')
    parser.add_argument('--hermes-arg', action='append', default=[],
                        help='Extra flag to pass to hermes (repeatable)')
    parser.add_argument('--gen-only', action='store_true',
                        help='Only generate JS harness, do not run')
    args = parser.parse_args()

    wast_file = os.path.abspath(args.wast_file)
    basename = os.path.splitext(os.path.basename(wast_file))[0]

    with tempfile.TemporaryDirectory() as tmpdir:
        json_file = os.path.join(tmpdir, basename + '.json')

        # Step 1: Convert .wast to JSON + .wasm files
        wast2json_cmd = [
            args.wast2json,
            '--enable-all',
            wast_file,
            '-o', json_file
        ]
        if args.verbose:
            print(f"Running: {' '.join(wast2json_cmd)}", file=sys.stderr)

        result = subprocess.run(wast2json_cmd, capture_output=True, text=True)
        if result.returncode != 0:
            # Report as a clean FAIL rather than producing no output (which
            # the test runner would report as CRASH).
            print(f"wast2json failed: {result.stderr}", file=sys.stderr)
            print("PASSED: 0")
            print("FAILED: 1")
            print("SKIPPED: 0")
            print("SPEC TEST FAILED")
            return 1

        # Step 2: Read JSON spec
        with open(json_file) as f:
            spec = json.load(f)

        # Step 3: Generate JS harness
        js_code = generate_js_harness(spec, tmpdir)
        js_file = os.path.join(tmpdir, basename + '_test.js')
        with open(js_file, 'w') as f:
            f.write(js_code)

        if args.gen_only:
            print(js_code)
            return 0

        if args.verbose:
            print(f"Generated JS harness: {js_file}", file=sys.stderr)
            print(f"Commands: {len(spec['commands'])}", file=sys.stderr)

        # Step 4: Run with hermes
        # Use -O0 for the JS harness to avoid slow compilation of large
        # generated test scripts. This only affects the JS compiler; wasm
        # modules are compiled with full optimizations independently.
        hermes_cmd = [
            args.hermes,
            '-O0',
            '--test262',
            '-Xhermes-internal-test-methods',
        ] + args.hermes_arg + [js_file]
        if args.verbose:
            print(f"Running: {' '.join(hermes_cmd)}", file=sys.stderr)

        result = subprocess.run(hermes_cmd, capture_output=True, text=True,
                                timeout=120)

        output = result.stdout
        if args.verbose or result.returncode != 0:
            if result.stderr:
                print(result.stderr, file=sys.stderr)

        # Print output
        print(output, end='')

        # Check for SPEC TEST PASSED in output
        if "SPEC TEST PASSED" in output:
            return 0
        elif "SPEC TEST FAILED" in output:
            return 1
        else:
            # Runtime error or crash
            print(f"hermes exited with code {result.returncode}", file=sys.stderr)
            if result.stderr:
                print(result.stderr, file=sys.stderr)
            return 1


if __name__ == '__main__':
    sys.exit(main())

# Compiling Wasm through shermes and the SH native backend

**Status:** design, revision 4, ready for implementation planning.

Revision 1 was reviewed externally and had two critical defects: it entered
the native unit without an exception boundary, and it left in place a check
that rejects every native module. It also asserted an ABI change that was
wrong, and a registration scheme that was underspecified.

Revision 2 fixed those; a second review round closed seven of eight findings
and raised one new one. Revision 3 closes the last finding (registration
versus lookup ordering) and corrects a claim revision 2 introduced: the
exception adapter covers less than it said, and the test proposed for it does
not exercise it.

Revision 4 applies three wording corrections from the third review round. All
findings across three rounds are closed.

## What this is, and what it is not

`hermesc --wasm foo.wasm -emit-binary` produces `.hbc` that a JS program
loads with `WebAssembly.Module.fromHermesBytecode()`, and from there the
entire `WebAssembly` API works: instantiation, imports, exports, tables,
traps. The goal here is the same thing with a native artifact in place of
the bytecode — a Wasm unit compiled by `shermes` to an object file, linked
into an application, and reachable from that application's JS as an ordinary
`WebAssembly.Module`.

The generated *code* is not the interesting part, and that is the central
finding: the IR the Wasm frontend emits is already compilable by the SH
backend. What differs is how the top level is entered, and what the entry
has to promise to its C++ caller.

### The frontend needs no backend work

Three separate populations of instruction have to be checked, and only the
first was checked in revision 1.

**Directly emitted.** `WasmIRGen`, `WasmHelpers` and `WasmFrontend`
construct 38 instruction *builder families*, which expand to 59 concrete
`ValueKind`s once the binary and floating-point operator families are
enumerated. Every one has a generator in `lib/BCGen/SH/SH.cpp`.
`WasmHelpers.cpp` contributes only `CallBuiltinInst`; `WasmCompile.cpp`
constructs no instructions of its own.

**Introduced by optimization.** `TypeOfIsInst`, `GetClosureScopeInst`,
`ResolveScopeInst` and `PrStoreInst`. SH implements all but
`ResolveScopeInst`, which lowers to the supported `LIRResolveScopeInst`.

**Introduced by lowering.** `LIRGetGlobalObjectInst`, `LIRLoadConstInst`,
`LIRAllocObjectFromBufferInst`, `MovInst` and `ImplicitMovInst`, all with
generators.

Two generators are fatal-unless-lowered and both are reached only through a
lowering pass that runs unconditionally: `SwitchInst` in `lowerModuleIR`
(`SH.cpp:2664`), and `StoreStackInst` in `lowerAllocatedFunctionIR`
(`SH.cpp:2695`) — *after* register allocation, not with the module passes.
Phis become moves during allocation.

The unsupported HBC call, compare and immediate-switch generators exist but
are fatal; the normal SH pipeline does not introduce them. Their presence in
the file is not evidence of anything.

One directly emitted family has an unimplemented case:
`BinaryExponentiationInstKind` reaches `unimplemented()` at `SH.cpp:1187`.
The Wasm frontend never emits `**`, so it does not bite, but the family is
not unconditionally supported.

Wasm operations are **supported ordinary IR plus private runtime-helper
calls** — not, as revision 1 claimed, uniformly builtin calls. `i32.add` is
an `FBinaryMathInst` followed by `AsInt32Inst` (`WasmIRGen.cpp:3699`),
floating negation is `FUnaryMathInst` (`WasmIRGen.cpp:5343`), and unaligned
loads are property loads and shifts (`WasmIRGen.cpp:6859`). There are 79
`WASM_BUILTIN` entries in `Builtins.def`. Where a `CallBuiltinInst` *is*
emitted, SH lowers it through `_sh_ljs_call_builtin` (`SH.cpp:2115`,
`StaticH.cpp:525`), which resolves through `getBuiltinCallable` with no
public-only restriction, so the private Wasm builtins are reachable
unchanged.

Intrinsics arrive as ordinary property loads off the global `HermesInternal`
(`WasmIRGen.cpp:6791`). The WebAssembly JSLib is part of
`hermesVMRuntime_obj`, so it is in both the full and lean VM libraries.

## The core change: a second artifact kind

`WasmModuleData` holds the compiled artifact behind a `WebAssembly.Module`.
It grows an alternative:

```cpp
struct WasmModuleData {
  std::vector<ExportDesc> exportDescs;
  std::vector<ImportDesc> importDescs;
  std::shared_ptr<hbc::BCProviderBase> bytecodeProvider;  // HBC path
  SHUnitCreator unitCreator = nullptr;                    // native path
};
```

Exactly one is set, and that invariant is enforced rather than assumed.
`WasmModuleData.h` advertises no VM dependencies and `SHUnitCreator` is
declared in `static_h.h`, so the typedef is repeated locally rather than
pulling a VM header into the frontend struct. It is a plain C
function-pointer type.

### The consumers, accurately

Two sites *execute* the artifact:

| site | when |
|---|---|
| `createModuleFromBytes`, run at `WebAssembly.cpp:669` | once per Module |
| `instantiateModuleImpl`, run at `WebAssembly.cpp:812` | once per Instance |

Revision 1 named `createAndBuildModule` (`WebAssembly.cpp:1010`), which is a
wrapper around the first, not the site.

Three further sites *consume* `WasmModuleData` without executing it:
`Module.exports` (`WebAssembly.cpp:1177`), `Module.imports`
(`WebAssembly.cpp:1285`), and allocation accounting
(`JSWebAssemblyModule.cpp:60`, which touches only the two descriptor
vectors). None needs a native branch, because descriptors are populated
identically — but the design owes that statement rather than an assumption
that there were only two consumers.

### The check that rejects every native module

`instantiateModuleImpl` refuses a null `bytecodeProvider` before it runs
anything (`WebAssembly.cpp:785`):

```cpp
if (!moduleData->bytecodeProvider) {
  raiseLinkError(runtime, "module was not fully compiled");
  return ExecutionStatus::EXCEPTION;
}
```

Every native module has that pointer null. The check becomes "neither
artifact is set", and the exactly-one invariant is asserted alongside it.

### Entering the unit: the exception boundary

This is where revision 1 was wrong, and the mistake is not cosmetic.

`_sh_unit_init` reaches `sh_unit_run`, which calls
`NativeJSFunction::_legacyCall` directly (`StaticHUnit.cpp:153`) with no
guard. An exception thrown by `unit_main` reaches `_sh_throw_current`, which
`abort()`s when no SH handler is installed and otherwise longjmps
(`StaticH.cpp:363-370`). Either
outcome is wrong underneath `WebAssembly.cpp`, whose callers expect
`CallResult` propagation and hold live `LocalsRAII` and `GCScope` objects
whose destructors a longjmp would skip. Installing an outer SH handler does
not make the jump safe; it only relocates where the C++ frames are abandoned.

The native path therefore goes through an adapter over
`_sh_unit_init_guarded` (`StaticHUnit.cpp:50`), which installs the jmpbuf
itself and reports failure by return value:

```cpp
/// Run \p creator's unit main and return its result, converting an SH
/// exception into ExecutionStatus::EXCEPTION.
CallResult<HermesValue> runNativeWasmUnit(Runtime &runtime,
                                          SHUnitCreator creator) {
  SHLegacyValue resOrExc;
  if (!_sh_unit_init_guarded(getSHRuntime(runtime), creator, &resOrExc)) {
    // _sh_catch returns _sh_get_clear_thrown_value, so the runtime no
    // longer holds the exception; put it back for the CallResult caller.
    runtime.setThrownValue(HermesValue::fromRaw(resOrExc.raw));
    return ExecutionStatus::EXCEPTION;
  }
  return HermesValue::fromRaw(resOrExc.raw);
}
```

The success value is a GC pointer and nothing roots it, so callers pin it
into a `PinnedValue` before the next allocation. `Runtime::runInternalJavaScript`
(`Runtime.cpp:1280`) is the existing instance of this same pattern.

**What this does and does not cover.** The adapter guards `unit_main` only.
Wasm traps and link errors are raised inside `__wasm_instantiate__`, including
the start call (`WasmIRGen.cpp:2344`), and that function is reached through
`Callable::executeCall1` (`WebAssembly.cpp:855`), whose native-function path
already installs its own `SHJmpBuf` around the call (`Callable.cpp:60-62`).
Those errors therefore never reach this adapter and are already handled.

What remains for the adapter is the top level itself, which builds the module
info object. The concrete case is SH-signaled stack exhaustion rather than a
Wasm-level error. Allocation failure is *not* an example: GC OOM either calls
`hermes_fatal` or throws a C++ `JSOutOfMemoryError` depending on
`HERMESVM_EXCEPTION_ON_OOM` (`GCBase.cpp:948`), and neither travels as a JS
exception through SH. The path is narrow, but leaving it unguarded means
`abort()` instead of an exception, so it is guarded. The verification section
says how it is tested, and is honest about the difficulty.

With that in place, both execution sites branch between
`runtime.runBytecode(...)` and `runNativeWasmUnit(...)`, and everything
downstream of the module info object is untouched.

### Why the descriptors come along for free

`createModuleFromBytes` does not read export and import descriptors out of
the `.wasm` bytes. It runs the top level and calls
`extractDescriptorsFromModuleInfo` on the object it returns
(`WebAssembly.cpp:695`). A native unit returns the same object, so it
supplies descriptors by the same route with no separate metadata channel.

(`compileWasmToModuleData` does build descriptors directly from parsed
metadata, but this route discards those and reconstructs them from the
executed object. That is what makes the native path a drop-in.)

### Instance isolation

Revision 1 justified this wrongly, claiming each instance needs the top
level re-run for fresh closures. It does not.

`WasmIRGen.cpp:886-889` states the split: `__wasm_instantiate__` holds
"import resolution, closures, memory views, tables, globals, trampolines",
while "the top-level function will just return a module info object with an
instantiate closure and descriptor arrays". `tlScope_` is a
`CreateScopeInst` inside `instantiateFunc_` (`WasmIRGen.cpp:890-906`), which
SH turns into a fresh `Environment::create` (`StaticH.cpp:568`) on every
call. Memories, globals, tables, closures and segment state belong to that
environment.

So per-instance state is isolated by `instantiate()` creating its own scope,
not by anything the unit does. `_sh_unit_init`'s re-run behaviour
(`StaticHUnit.cpp:105`) — reuse the registered unit, re-run its main — is
consistent with this but is not what provides isolation. The shared
unit-level caches hold hidden classes and symbols, not instance state.
`SHUnit::moduleExports` is shared, but the Wasm frontend does not generate
the require path that uses it.

**Considered and not taken:** since isolation comes from `instantiate`, the
native path could run the unit *once* at Module-creation time and keep the
`instantiate` closure, so each Instance only calls it. That would avoid
re-running the unit entirely. It needs a GC-visible home for the closure —
`JSWebAssemblyModule` holds only a `unique_ptr<WasmModuleData>`, a plain C++
struct with no GC slot, so it would need an internal property or a new
`GCPointer` and mark function. Deferred: it is an optimization, and matching
the bytecode path's shape is worth more than saving a unit re-run.

## Registration and lookup

A linked unit registers itself. The registration node is statically
allocated in the unit's own object file, so registration allocates nothing:

```c
typedef struct SHWasmUnitReg {
  const char *name;
  SHUnitCreator creator;
  struct SHWasmUnitReg *next;
} SHWasmUnitReg;

/* generated into mymod.c, beside sh_export_mymod */
static SHWasmUnitReg s_wasm_reg = {"mymod", sh_export_mymod, NULL};
__attribute__((constructor)) static void _sh_wasm_reg_ctor(void) {
  _sh_wasm_register_unit(&s_wasm_reg);
}
```

The name is the `--exported-unit` value, taken from `options.unitName` at
generation time. It is **not** `SHUnit::unit_name`, which the backend
hardcodes to `"sh_compiled"` (`SH.cpp:3184`).

This block is emitted only for Wasm units. `--exported-unit` also serves
ordinary JS units, which must not acquire a Wasm registration.

MSVC has no `__attribute__((constructor))` and needs the `.CRT$XCU` section
declaration instead.

JS reaches a registered unit through `WebAssembly.Module.fromNativeUnit(name)`.

### The linking requirement

A constructor only runs if its object file is in the link, and **a JS string
lookup creates no native reference to `sh_export_mymod`**. This repository
already documents the consequence, in `WasmCompileStub.cpp:19-26`:

> a static link only extracts an archive member in order to resolve an
> *undefined* symbol

So an archived Wasm unit is never extracted and never registers. The
supported arrangement is therefore stated, not left implicit:

- **Link the unit object directly.** This is what `shermes -c` produces and
  is the intended use.
- If units are collected into a static archive, the application must force
  inclusion (`--whole-archive` / `-force_load`), or link against a generated
  manifest object that names every creator so the references are undefined
  and the members get extracted.

### Concurrency and lifetime

Static allocation supplies neither thread safety nor lifetime management, so
both are specified:

- **Duplicate names are rejected at registration**, not assumed impossible.
  The linker-collision argument covers one ordinary static link; it does not
  cover independently linked shared libraries loaded later, and `shermes`
  already builds shared objects (`compile.cpp:467`).
- **Lookup is synchronized, not only insertion.** A `dlopen` on another
  thread can push while a lookup walks the list.
- **Initialization order is defined.** The list head is zero-initialized in
  `.bss` before any constructor runs, and the registry lock is a
  constant-initialized primitive so it is usable from a static constructor.
- **Lookup ordering is defined by policy: lookup is not a
  static-initialization-time operation.** This is a supported-usage rule, not
  a technical impossibility — nothing stops an embedder creating a runtime and
  evaluating JS from a static constructor, since `makeHermesRuntime` and
  `evaluateJavaScript` impose no such precondition. But a `fromNativeUnit`
  reached that way may legitimately miss a unit whose constructor has not yet
  run, and that is not supported. In the ordinary case JS runs after `main`
  begins, by which point every static constructor in the program image has
  run; a unit brought in by `dlopen` is visible once `dlopen` returns, because
  the library's constructors run before it does. Registration is the only
  operation allowed at static-initialization time.
- **The lock is released before the unit is executed.** Running a unit
  re-enters the VM and must not happen under the registry lock.
- **A registered image must stay loaded.** The registry holds pointers into
  it, and SH additionally retains lazy string sources and native function
  pointers from an initialized unit; its existing contract already forbids
  unloading one before the runtime is destroyed (`static_h.h:72`,
  `StaticHUnit.cpp:165`). The registry is process-global and can outlive any
  one runtime, which widens that requirement rather than narrowing it. There
  is no unregistration.

### Why that entry point is not config-gated

`fromHermesBytecode` is gated on `enableUntrustedBytecodeFromJS` because JS
supplies the bytes. `fromHermesURL` is ungated, and its comment gives the
reason: the embedder installing a resolver is itself the authorization. A
self-registered unit is stronger still — the unit was linked into the binary.
Script can name a linked unit but cannot introduce one.

Two consequences to document rather than discover: registration exposes a
unit to *every* runtime in the process, since the registry is process-global;
and `HERMES_ENABLE_WASM` remains a build prerequisite.

## Data segments

`wasmDataSegmentInit` (`HermesBuiltin.cpp:1286`) reaches the data blob by
walking the stack to the caller's `CodeBlock` and from there its
`RuntimeModule`. SH-compiled functions are `NativeJSFunction`s, so
`getCalleeCodeBlock()` returns null and the builtin raises "Cannot be called
from native code".

This is the only such dependency in the Wasm path. Within
`HermesBuiltin.cpp` the only other `getCalleeCodeBlock` caller is
template-object construction, which this frontend does not emit. Beyond
`wasmDataSegmentInit` itself (`HermesBuiltin.cpp:1307`), the Wasm builtins,
the `JSWebAssembly*` classes and `WebAssembly.cpp` add no further calls.
`getCalleeCodeBlock` is common elsewhere in the VM — error stacks, the
debugger, the profiler, the interpreter — and those paths accommodate native
frames, which is the substantive point. Function branding accepts any `Callable`, live
globals go through `Callable::executeCall*`, and error-stack recording
handles native frames.

The scope is **nonempty** data segments. An empty segment never reaches the
builtin (`WasmIRGen.cpp:2076`, `WasmIRGen.cpp:2243`), and the builtin itself
returns before the stack walk when length is zero
(`HermesBuiltin.cpp:1299`). So a module whose only segments are empty
already works.

The fix follows an existing precedent in the same backend. `SH.cpp:1716`
says of template objects:

> Can't lower to calling the HermesBuiltin because that depends on
> RuntimeModule, so we have a `_sh_get_template_object` function instead,
> which can read from the SHUnit templateMap.

So: `SHUnit` carries the blob (see the ABI section for how), the SH backend
emits `Module::getBinaryDataStorage()` as a `static const uint8_t[]`, and
`generateCallBuiltinInst` special-cases the builtin id into
`_sh_wasm_data_segment_init(shr, shUnit, ...)` — the existing builtin body
with the stack walk replaced by a unit lookup. That function already
special-cases `Math_sqrt`, so the shape is not new.

### The alternative that was considered and rejected

Encoding segment bytes as string literals would serve both backends with no
SHUnit change and no backend special case.

The cost is in the emitted representation. SH classifies each string by
`isAllASCII` (`SH.cpp:113`) and stores it in `ascii_pool` or `u16_pool`, the
string table's offset encoding reserving the high bit to say which
(`static_h.h:100`). Under the obvious encoding of one source byte per code
unit, a segment containing any byte ≥ 0x80 lands in the UTF-16 pool at two
bytes per byte, and every segment needs a per-byte decode loop in place of a
`memcpy`. An all-ASCII segment would stay one byte per byte, and a packed
encoding could do better than the obvious one — this is a cost argument
about the straightforward scheme, not a proof that no string encoding could
work.

Worth recording that this was a live option rather than a foregone one:
`binaryDataStorage` was introduced by `714a60564` — the Wasm commit itself —
and Wasm is its only consumer, so replacing it would not have disturbed any
pre-existing facility.

## The SHUnit ABI

Revision 1 said `SHUnit` "gains `binary_data` and `binary_data_size` beside
the existing `obj_key_buffer`". That is an ABI break, and so is the obvious
correction.

Inserting fields shifts everything after them, including `unit_main` and
`runtime_ext` (`static_h.h:126`). Appending does not rescue it either:
generated C allocates its own `UnitData` embedding the `SHUnit` layout it was
compiled against (`SH.cpp:3150`), and runtime code then reads that allocation
using *its* layout. An object file built against an older header
under-allocates for a newer runtime. `_SH_MODEL` catches heap-model
mismatches but encodes no `SHUnit` version (`static_h.h:182`,
`lib/CMakeLists.txt:334`), so a mismatched pair links cleanly and fails at
runtime.

Two acceptable resolutions, in order of preference:

1. **Version the layout.** Extend the `_SH_MODEL` symbol name with an
   `SHUnit` ABI version so a stale object fails at link time with an
   undefined symbol rather than at runtime with a corrupt read. Then append
   the fields; designated initializers zero them for non-Wasm units, which is
   the correct value.
2. **Keep the blob out of `SHUnit`.** Put the Wasm metadata in a separately
   versioned structure the unit points at, leaving the `SHUnit` layout alone.

Either way the design requires generated units and runtime libraries to be
built together, and says so.

## shermes CLI

- Accept `.wasm` by extension, as `CompilerDriver.cpp:1026` already does for
  `hermesc`, plus an explicit `-wasm`.
- Build the Module with `compileWasmModule` on shermes' own `Context`,
  skipping parse, sema and IRGen.
- **Keep the optimization stage.** It runs in `shermes.cpp:1011`, *outside*
  `shermesCompile`, so a frontend branch that jumps straight to
  `shermesCompile` would silently skip it.
- `--exported-unit=NAME` is what makes the result linkable. It already
  refuses `Executable` and `Run` output levels (`shermes.cpp:918`), which is
  correct here.

A standalone `shermes foo.wasm -o foo` producing a runnable executable is out
of scope. It needs a separate `emitMain` variant and is not what "linkable
into an app" asks for.

## Verification

Staged, and the staging is deliberate rather than a shortcut.

**First: the e2e subset.** Add native RUN lines to the `test/wasm` e2e tests
that exercise the paths most likely to diverge — multi-value returns,
`call_indirect`, `try`/`delegate`, bulk memory, and above all the
data-segment tests, which cover the only known blocker. A trap test is
required, but for what it actually covers: it exercises the *instantiate*
path, which `Callable` already guards, and so it confirms that traps surface
as exceptions rather than that the new adapter works.

The adapter is exercised only by a unit whose `unit_main` throws, which a
well-formed Wasm module does not readily produce — the top level allocates a
handful of objects. This is covered: `test/wasm/native-unit-throws.wat`
registers an ordinary JS unit (`native-unit-throws-throwing.js_`, which
throws at its top level) under a Wasm unit name via a hand-written C shim
(`native-unit-throws-shim.c`, calling `_sh_wasm_register_unit` directly,
since a non-Wasm unit does not self-register), and reaches it through
`fromNativeUnit`. The test asserts both that the exception surfaces as a
catchable `Error` and that a subsequent `fromNativeUnit` on a good unit still
works afterwards — the latter is what shows `setThrownValue` restored the
runtime's state rather than leaving it wedged.

**Then: the spec suite.** `test/wasm/spec/run-spec-test.py` turns one `.wast`
into many `.wasm` modules and loads each at runtime with
`fromHermesBytecode`. Natively each module needs its own compile-and-link
step, so a suite that is one process today becomes hundreds of builds. Worth
doing, and worth pricing separately once the path works.

Until the spec suite runs natively, this path is not "fully verified" and
should not be described as such.

## Out of scope

- A JS unit consuming a Wasm module compiled in the same run. Needs a
  multi-unit wiring story that does not exist yet.
- `new WebAssembly.Module(bytes)` inside a shermes-built binary. Runtime
  `.wasm` compilation already works against the full VM; the lean VM
  substitutes a rejecting stub, and shermes' `--lean` defaults to off. What
  is out of scope is guaranteeing it, since it pulls the whole Wasm frontend
  and wabt into the output.
- A standalone executable from a `.wasm`, per the CLI section above.

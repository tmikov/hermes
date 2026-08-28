# Wasm bytecode loading and bytecode-trust gates — design

Date: 2026-08-15
Status: approved (brainstorm), pending implementation plan
Branch: `wasm-old-rebased`

## Context

Hermes is an AOT engine. Compiling `.wasm` at runtime is slow and
memory-hungry, so the practical way to ship a WebAssembly module is to
precompile it to Hermes bytecode (`.hbc`) and load that. Runtime `.wasm`
compilation survives only as an eval-class convenience: supported, discouraged,
and configurable out of the binary (like `eval`). **Loading precompiled `.hbc`
is the actual feature.**

Hermes bytecode is trusted input by design. Making the bytecode loader robust
against hostile bytecode would require redesigning the bytecode format, so that
is out of scope and non-negotiable: whoever loads `.hbc` is trusted to have
produced it.

The current implementation has one entry point, `createModuleFromBytes` in
`lib/VM/JSLib/WebAssembly/WebAssembly.cpp`, that decides `.hbc`-vs-`.wasm` by
**content-sniffing the bytes** (`BCProviderFromBuffer::isBytecodeStream`).
Because `WebAssembly.compile` / `new WebAssembly.Module` /
`WebAssembly.instantiate` are reachable from ordinary JS with caller-supplied
bytes, a script can hand them crafted bytes that sniff as `.hbc` and get
executed as trusted bytecode — full VM control from plain JS (review finding
§3.3).

The Worker has the same class of defect, but **not in this branch's current
tree.** On `origin/static_h`, `new Worker(input)` accepts an `ArrayBuffer` /
`TypedArray` / `DataView` and copies the raw bytes verbatim into
`evaluateJavaScript`, which content-sniffs — so a JS-supplied buffer whose bytes
are a `.hbc` image executes as bytecode. This branch still carries an older,
string-only worker (`args[1].asString(rt).utf8(rt)`); because the bytecode magic
`0x1F1903C103BC1FC6` is not valid UTF-8, that path cannot carry bytecode and is
not exploitable. The branch cannot rebase onto `static_h` yet (it is mid-merge
between two divergent Wasm lines), so the vulnerable worker is not present to fix
here. See "Worker fix" below.

The defect is that a single entry infers *trust level* from
attacker-controllable bytes. The fix makes trust a deliberate, caller- or
embedder-chosen property, never inferred.

## Goals

- Untrusted JS can never cause `.hbc` execution by default.
- Precompiled `.hbc` remains loadable — the primary feature — through paths
  whose trust is explicit.
- All JS-facing surface lives under `WebAssembly`, for discoverability.
- Design `EnableUntrustedBytecodeFromJS` so it also covers the Worker's
  binary-script path once the `static_h` worker is merged in (the fix itself is
  deferred — that worker is not in this tree yet).

## Non-goals

- Hardening the bytecode loader against hostile bytecode (impossible without a
  format redesign; `.hbc` stays trusted by definition).
- Making runtime `.wasm` compilation fast or mandatory (it stays optional /
  eval-class / compilable-out).
- Internal slots / brand checks for the Wasm linking ABI (review §4.4) — a
  separate, later effort.

## Trust model: two independent gates

Two `RuntimeConfig` flags, each with a matching CLI option, both defaulting to
`false`. They follow the existing `RUNTIME_FIELDS` pattern in
`public/hermes/Public/RuntimeConfig.h` (e.g. `EnableEval`), read on the VM
`Runtime` like `runtime.enableEval`.

| flag | default | governs |
|---|---|---|
| `EnableUntrustedBytecodeFromJS` | `false` | JS causing **untrusted (JS-supplied)** bytecode to load: `WebAssembly.Module.fromHermesBytecode(bytes)`, and the Worker running a bytecode string |
| `EnableWasmBytecodeContentSniffing` | `false` | `WebAssembly.Module(bytes)` / `compile` / `instantiate` auto-detecting `.hbc` vs `.wasm` from the bytes |

The gates are independent capabilities, not a hierarchy, with one composition
rule (below). **Neither gate governs the embedder URL route** — that is
authorized solely by the embedder having provided bytes for the URL (a registry
entry or resolver).

Rationale for the names: the dangerous property of the bytes path is that the
bytecode is *untrusted* (JS supplies it), hence `EnableUntrustedBytecodeFromJS`
rather than a Wasm-specific name — it also governs the Worker. The sniffing gate
is specific to the Wasm entry points, hence `EnableWasmBytecodeContentSniffing`.

## JS surface (all under `WebAssembly`)

### Spec entries — always `.wasm` by default

`new WebAssembly.Module(bytes)`, `WebAssembly.compile(bytes)`,
`WebAssembly.instantiate(bytes, imports)`:

- **`EnableWasmBytecodeContentSniffing` off (default):** the bytes are always
  treated as `.wasm`. Bytes that look like `.hbc` produce a `CompileError`,
  never silent bytecode execution. This alone closes §3.3.
- **`EnableWasmBytecodeContentSniffing` on:** the entry sniffs. Detected `.hbc`
  is loaded **only if `EnableUntrustedBytecodeFromJS` is also on** — the bytes
  are JS-supplied and therefore untrusted, so loading them needs that gate too.
  With sniffing on but untrusted-bytecode off, detected `.hbc` is **refused**
  with a `CompileError`.

Removing the unconditional sniff from `createModuleFromBytes` is the core
security change; sniffing becomes a gated branch.

### `WebAssembly.Module.fromHermesBytecode(bytes) -> Module`

Explicit load of caller-supplied Hermes bytecode. No sniffing — the caller has
declared the bytes are bytecode. Gated by `EnableUntrustedBytecodeFromJS`;
throws if that flag is off. This is the eval-class path: enabling it is an
embedder decision to trust JS-supplied bytecode.

### `WebAssembly.Module.fromHermesURL(url) -> Module`

Resolves `url` (a string) to trusted Hermes bytecode through the embedder (see
the embedder API below) and loads it. JS never supplies or sees the bytes, so
there is nothing to falsify — the `.hbc` is unfalsifiable from JS. The resolved
buffer is **always `.hbc`** and is loaded as bytecode directly; this route never
compiles `.wasm` and never sniffs (there is nothing to sniff — the route is
bytecode by definition).

**Not config-gated.** Authorization is the embedder having provided bytes for
the URL (a registered resolver and/or registry entry); with neither, the call
throws.

### Notes on the surface

- Both new entries are static factories on `WebAssembly.Module`, analogous to
  `Array.from`, and synchronous (the embedder resolver is synchronous and the
  existing `compile`/`instantiate` already resolve their Promises
  synchronously).
- Instantiation stays `new WebAssembly.Instance(module, imports)`. No new
  `compile*`/`instantiate*` variants — the two factories are the entire new JS
  surface (YAGNI).

## Embedder API (#1): trusted Wasm bytecode by URL

The embedder supplies trusted Hermes bytecode for a URL through one of two
mechanisms — a **registry** (the convenient common case) or a **resolver** (full
control). Both deal only in `.hbc`. Everything here is trusted: bytes are
loaded as bytecode without validation, so a bad registration is an embedder bug,
not a security issue. This is a Wasm-specific facility, separate from the
worker's script resolver (different consumers, different lifetimes).

The facility lives on an ICast interface on the runtime (working name
`IWasmModuleProvider`, its own UUID), mirroring `ISetWorkerSetup` in spirit:

- **Registry (convenience):**
  `void registerWasmBytecode(std::string url, std::shared_ptr<const jsi::Buffer>
  bytecode)`. The embedder chooses the URL (e.g. `"app://physics.wasm"`),
  bundles the precompiled module, and JS loads it with
  `WebAssembly.Module.fromHermesURL("app://physics.wasm")`. The URL is the same
  string both sides already know at build time, so nothing needs to be plumbed
  back from registration. The buffer is `.hbc` (not checked) and is referenced
  for the life of any module built from it, so it must remain valid accordingly.

- **Resolver (full control):** an optional integrator-implemented
  `IWasmModuleResolver : public jsi::ICast` with
  `virtual std::shared_ptr<const jsi::Buffer> resolve(const std::string& url,
  std::string& error) = 0;`, registered via a setter on the same interface
  (`setWasmModuleResolver(jsi::ICast*)` / `getWasmModuleResolver()`), stored
  opaquely as `jsi::ICast*` for ABI stability. Returns trusted `.hbc`, or
  `nullptr` with `error` set to decline.

**Precedence — resolver first, registry fallback.** `fromHermesURL(url)`:

1. If a resolver is registered, call it; if it returns a buffer, use it.
2. Otherwise (no resolver, or resolver returned `nullptr`), look `url` up in the
   registry.
3. If neither yields bytes, throw.

So a resolver can override or intercept any URL, and the registry serves
whatever the resolver declines — a resolver never has to re-implement the simple
cases it doesn't care about.

## Worker fix — DEFERRED to after the merge/rebase

The vulnerable worker (the `ArrayBuffer`/`TypedArray`/`DataView` path that copies
raw JS-supplied bytes into `evaluateJavaScript`) lives on `origin/static_h`, not
in this branch's current tree, and the branch cannot rebase onto `static_h` yet
(mid-merge between two divergent Wasm lines). There is nothing to fix here now:
this branch's string-only worker cannot carry bytecode (the magic is not valid
UTF-8).

`EnableUntrustedBytecodeFromJS` is designed to cover it. **Once this work lands
and the branch rebases onto the `static_h` worker**, that flag must gate the
worker's binary-script path: when off (default), a Worker input buffer whose
bytes are Hermes bytecode (`isHermesBytecode`) is **refused**; when on, it may
run as bytecode. This is a `isHermesBytecode`-guard at the point `startWorker`
receives its `std::string script` from `copyBufferBytes`, plus propagation of the
flag from the parent runtime into the worker's `RuntimeConfig`. It must not
disturb the embedder's own trusted `evaluateJavaScript(appBytecode)` (the
embedder's direct call is trusted; only the worker's JS-supplied buffer is not).

This is tracked as a follow-up, not a task in the accompanying plan.

## Behavior matrix

`WebAssembly.Module(bytes)` with `.hbc`-looking bytes:

| sniffing | untrusted-from-JS | result |
|---|---|---|
| off | off | `CompileError` (treated as `.wasm`, fails to validate) |
| off | on | `CompileError` (still treated as `.wasm`) |
| on | off | `CompileError` (detected `.hbc`, refused) |
| on | on | loads as bytecode |

`WebAssembly.Module.fromHermesBytecode(bytes)`: loads iff
`EnableUntrustedBytecodeFromJS` on, else throws — independent of sniffing.

`WebAssembly.Module.fromHermesURL(url)`: loads the embedder's trusted `.hbc` iff
the embedder provided bytes for the URL (resolver or registry), independent of
both flags.

Worker (after the `static_h` merge, deferred): a binary Worker input whose bytes
are Hermes bytecode runs as bytecode iff `EnableUntrustedBytecodeFromJS` on;
otherwise refused. Not implemented in this branch's plan — the vulnerable worker
is not in this tree yet.

## Test-driver migration

The `.hbc` review (§3.3) noted that #15 migrated the Wasm lit-test drivers onto
auto-detecting `WebAssembly.Module(hbcBytes)`. With sniffing off by default those
drivers break. They move to the explicit trusted paths — `hermescli.loadHBC`
(already gated behind `-Xhermes-internal-test-methods`), or a test build that
sets the new flags / registers a resolver — depending on what each driver
exercises. The lit runner enables whichever flag a given test needs via the CLI
options.

**Real embedders, not just tests.** Defaulting the spec entries to `.wasm`-only
is a behavior change for any embedder currently relying on `WebAssembly.Module`
auto-detecting `.hbc` (the production path today). Such embedders migrate to
`WebAssembly.Module.fromHermesURL` (register the bytecode) or
`fromHermesBytecode` (with `EnableUntrustedBytecodeFromJS`), or set both existing
flags to keep the old behavior. This is intended and is the point of the change.

## Open items / future

- Internal slots and brand checks for the `__wasm_*` linking ABI (review §4.4)
  remain a separate later effort; unrelated to this change.
- The exact CLI option spellings and their wiring in `RuntimeFlags.cpp` /
  the CLI driver are settled in the implementation plan.

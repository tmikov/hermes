# The `hermes-node` branch

**What this file is:** the branch's own charter. It exists because the most
important fact about `hermes-node` is invisible from its contents — the
branch is an *integration* branch and is **not intended to be upstreamed**.
Nothing in the tree says so, and a reader who assumes otherwise will make
the wrong call about where to put a change.

---

## What it is

`hermes-node` integrates the Hermes-side work for hermes-node. It carries
merges, and its history is expected to be a merge history rather than a
clean series of patches.

Created 2026-08-30 from `static_h` @ `26ec0a0c3`.

## Merge direction

`static_h` → `hermes-node`, only. The branch consumes upstream; it never
feeds back. There is no plan to land it, rebase it onto a landing branch,
or open it as a pull request, so the usual reasons to keep a history
bisectable or a series reviewable do not apply here.

## Where a change belongs

This is the operational consequence, and the reason the charter is worth
writing down.

A change that *should* eventually reach upstream must not be authored
here. Branch it off `static_h`, keep it as a reviewable series there, and
merge it in. It then exists in a form that can be sent upstream
independently of this branch's fate.

The worked example is already in this history: the two debugger commits
(`fbbd99f17`, `b66ffa900` — read-only bytecode page breakpoints, and
`ICancelAsyncTimeout`) live on `hermes-node-fixes`, branched from
`static_h` and rebased onto it before being merged here as a
fast-forward. That branch remains the upstreamable artifact; this one is
where it gets used.

Only integration itself — merges, conflict resolution, glue that is
meaningless outside this combination — should originate on `hermes-node`.

## Related

- Open NAPI follow-ups are tracked in `dz/` (component `napi`), filed on
  the `n-api-todo` branch.
- The NAPI implementation itself is upstream on `static_h` under
  `API/napi/`; the retired `old-n-api` branch was its development history
  and holds nothing that `static_h` lacks.

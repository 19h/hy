# HCLI 0.26.4 parity review

The comparison covers upstream `92df5b10c8b2e860a10bbfd8595c8bc2b7c3b64f`
through `c10908cfb864008fcdb6ec067cb98a6ac67c7da2`, using the local
`../ida-hcli` checkout as the primary source. Hy started at `cd8b661` with
existing, uncommitted GitHub transport, timestamp, and directory-packing work.
Those changes are retained. The upstream checkout is unchanged.

This report updates the [earlier port review](parity.md) for this commit range.
It does not replace that report's limits for older, unrelated behavior.
Hy retains its native executable, release identity, and configuration namespace.

## Assumption register

| ID | Assumption | Stress test or falsification probe | Dependent results |
|---|---|---|---|
| D1 | The supplied reflog entries identify the intended upstream range. | Verify both full SHAs and the upstream HEAD before and after the review. A later HEAD requires a separate delta review. | Feature coverage below. |
| D2 | Hy remains a standalone native process. | Inspect its executable entry point and extension subprocess boundary. An in-process IDAPython embedding would require a different source of authoritative IDA facts. | Python-only in-process detection and uvx self-update advice are inapplicable to Hy's executable. |
| D3 | HCLI's locked Python dependencies define the comparison profile. | Repeat source comparisons with the upstream lockfile, including wildcard matches, prereleases, arbitrary equality, and patch-version boundaries. | `requiresPython` comparisons use `packaging` 26.0; they do not infer behavior from an older PEP 440 implementation. |
| D4 | Test fixtures must operate within owned temporary paths. | Inspect isolated HOME, IDAUSR, configuration, subprocess recorders, and loopback servers. Check that upstream remains clean. | Integration tests exercise CLI behavior without installing plugins into the developer's IDA or changing desktop registration. |
| D5 | Platform compilation and native execution establish different facts. | Run Windows and Linux compilation checks; separately run installers, registry operations, and IDA on each native OS. | Cross-compilation validates conditional code. Native Windows/Linux runtime results remain unknown. |
| D6 | Expanded archive component metadata corresponds to the suite's files. | Exercise string and inline components, nested descriptors, missing entry points, name mismatches, depth limits, and installed-name collisions. | Suites share their root archive and lifecycle; authored lint rejects expanded objects. Files must also pass installation validation. |
| D7 | Repository catalogue identity and archive metadata have separate roles. | Supply catalogue keys and location descriptors whose version, host, or dependencies differ from the selected archive. Verify checksums and exact archive-name selection. | Selection uses catalogue identity; downloaded roots become fixed inputs to a second dependency plan before publication. Prepared local sources retain their identity check. |

## Change coverage

| Upstream behavior | Hy implementation |
|---|---|
| Recursive plugin dependencies, optional entries, host qualifiers, installed pins, and backtracking | Shared resolver in `src/plugin/resolve.rs`; fixed local roots, compatibility pruning, required-before-optional scheduling, dependency-order installation, and installed-host constraints. |
| Plugin suites and inline component metadata | `src/plugin/components.rs`; recursive component expansion, collision checks, suite Python requirements, scoped settings, installed status, and component-aware removal. |
| `requiresPython` and one full-version probe per command | `src/plugin/requirements.rs` and `src/ida/python/mod.rs`; PEP 440 validation and membership, cached success/failure, patch-version filtering, and unrestricted fallback when Python is unknown. |
| Per-platform and per-Python bundle selection | `src/cmd/plugin_bundle_sources.rs`; shared resolution for each cell, local archive/directory roots, bare and range specs, recursive dependencies, and archive reuse. |
| Standalone bundle wheel downloads | `src/plugin/bundle/download.rs`; `uv tool run --python VERSION pip download`, independent of an installed IDA. Parent Python module/startup variables are removed. |
| Optional wheel failures | `src/plugin/bundle/cell.rs`; isolated retry directories, required closure first, deterministic optional groups, failed-group removal, and atomic final bundle publication. Setup/I/O failures remain terminal. |
| Installed Python requirements constrain pip without becoming install requests | `src/cmd/plugin_ops.rs` and `src/ida/python/venv.rs`; temporary constraints for both dry-run and installation, extras removal with a warning, and invalid installed requirements skipped. |
| Aggregated repositories and skipped-repository warnings | `src/plugin/repositories.rs`; command-local merged identity catalogue retaining each archive's owner and bundle wheelhouse. Explicit root scopes restrict the root while dependencies span repositories. |
| Grouped search/status and qualified install hints | Shared listing helpers, repository fields in JSON, community-last ordering, local entries after repository groups, and retained host qualification in hints. |
| Repository-prefixed management and `extension` alias | `plugin` has the visible alias `extension`; uninstall accepts multiple references. Extension maintenance is under `hcli-extensions`. |
| Explicit setting defaults and missing-only installation prompts | Settings retain explicit responses equal to defaults; upgrades do not prompt again for already configured fields. |
| Lint errors fail the command | Validation errors return status 1; recommendations retain status 0. Component diagnostics, undeclared plugin warnings, and portal dependency suggestions are included. |
| IDA 9.5 metadata | Compatibility catalogue and the upstream schema are updated. |
| Newest-first download navigation | Numeric runs sort newest first; service packs precede their base, and prereleases follow it. |
| Invocation-aware follow-up commands | `src/util/invocation.rs`; explicit display-name override, verified PATH aliases, and shell-quoted executable paths. |
| Exact shell-profile variable matching and change previews | Shared profile matching distinguishes a variable from names sharing its prefix and displays removed/added lines before applying configuration. |
| Installer behavior and cleanup | Dry-run reflects conditional actions; existing invalid destinations fail; debug traces are removed; IDA Free Windows omits `--install_python`; Linux executes a private executable copy; failed installations remove their created destination. |
| Installation configuration and diagnostics | Empty, whitespace-only, and relative `ida-install-dir` values fail. Doctor/explain report a missing installation or `idat` before Python diagnostics. |
| Noninteractive license installation | Missing implicit destination fails; an explicit destination can be created without terminal input. |
| MCP installation | Repository-resolved plugin installation, uv/uvx checks, checked git availability, agent setup, and manual `mcp.json` output. |
| Idle OAuth browser preconnects | Existing concurrent callback handling already covers the upstream change. Listener teardown verification accounts for OS socket-close completion. |
| Drive-letter and stream-like archive paths | Archive and wheelhouse extraction reject colon paths and resolved destinations outside the extraction root, including existing symlink redirection. |

The immutable upstream sources include
[dependency resolution](https://github.com/hexrayssa/ida-hcli/blob/c10908cfb864008fcdb6ec067cb98a6ac67c7da2/src/hcli/lib/ida/plugin/resolve.py),
[component handling](https://github.com/hexrayssa/ida-hcli/blob/c10908cfb864008fcdb6ec067cb98a6ac67c7da2/src/hcli/lib/ida/plugin/components.py),
[bundle orchestration](https://github.com/hexrayssa/ida-hcli/blob/c10908cfb864008fcdb6ec067cb98a6ac67c7da2/src/hcli/commands/plugin/bundle.py),
[pip invocation](https://github.com/hexrayssa/ida-hcli/blob/c10908cfb864008fcdb6ec067cb98a6ac67c7da2/src/hcli/lib/ida/python/__init__.py),
and [Python environment handling](https://github.com/hexrayssa/ida-hcli/blob/c10908cfb864008fcdb6ec067cb98a6ac67c7da2/src/hcli/lib/venv.py).
Executable source adapters are retained beside the corresponding Rust tests.

## Resolution and publication boundaries

```text
repository snapshots + fixed local roots + installed identities
    -> discover reachable candidates
    -> filter platform / IDA / Python compatibility
    -> prune candidates with unsatisfied required dependencies
    -> search newest first, backtracking on conflicting constraints
    -> select and fetch verified root archives
    -> replan using the downloaded roots' metadata as fixed inputs
    -> produce dependency order and optional-skip diagnostics
    -> fetch verified dependencies and prepare each installation
    -> resolve/install Python requirements and publish plugin files
```

Let V be candidate versions, E their dependency edges, and P catalogue identities.
Version sorting costs `O(sum(v_i * log(v_i)))`. Reachability is a graph traversal
with identity lookup; the current vector-backed lookup costs `O(P)` per lookup.
Pruning takes at most V removal rounds, each checking remaining edges and
candidate lists. A conservative bound is `O(V * E * (P + V))`.

General constraint search has exponential worst-case time. Hy limits search to
10,000 processed requirements and 256 selection levels. It retains the graph
and cloned branch state; with graph storage G and selection depth N, a
conservative memory bound is `O(G * N)`. These are search limits, not a claim
that total catalogue parsing or archive storage is constant or bounded by them.

Resolution completes before installation starts. Each plugin replacement is
staged, and a completed bundle is published atomically. Installation of an
entire dependency graph is not a single transaction: an independently installed
dependency can remain if a later plugin fails. Optional-parent installation is
skipped when its required child failed. These boundaries follow the command's
dependency lifecycle rather than implying graph-wide rollback.

## Validation

Source comparisons use an isolated Python 3.13.16 environment synchronized to
the upstream lockfile. The older review's CPython 3.13.15 corpus pins are not
silently reinterpreted as new-runtime evidence.

The source comparisons pass for 128 dependency graphs, 507 Python-requirement
cases, 2,304 download argument plans, 36 download errors, 325 archive-selection
cases, 90 extraction projections, 93 wheelhouse cases, and 79 archive-index
cases. Platform configuration plans, profile changes and previews, pip plans,
lint CLI findings, repository bundle staging, and archive identity tests also
pass their targeted upstream comparisons. A corpus verifies its represented
inputs; it does not establish equivalence for arbitrary inputs or every
supported Python runtime.

| Check | Result |
|---|---|
| Serialized native suite, 63 test targets | 810 passed; 9 opt-in/manual tests ignored in this run. |
| Python extension compatibility against HCLI 0.26.4 | All 7 opt-in tests passed separately. |
| Release-profile background updater | The opt-in test passed separately. |
| Rust formatting and `git diff --check` | Passed. |
| Clippy across all targets with warnings denied | Passed. |
| Windows GNU target | Compilation check passed. |
| Linux x86-64 target | Debug executable built and linked with Zig. |
| Initial dirty checkout contents | All 17 recorded file hashes preserved after accounting for the intentional packing-function visibility and invocation-module export changes. Git index unchanged. |
| Upstream source | HEAD remains `c10908cfb864008fcdb6ec067cb98a6ac67c7da2`; checkout remains clean. |

Reproduction commands, with an isolated interpreter containing the upstream
project and its locked dependencies:

```sh
cargo test --all-targets --no-fail-fast -- --test-threads=1
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings

HY_TEST_EXTENSION_PYTHON=/path/to/oracle/bin/python \
    cargo test --test extensions -- --ignored
cargo test --release --test background_update -- --ignored

cargo check --target x86_64-pc-windows-gnu
cargo zigbuild --target x86_64-unknown-linux-gnu
```

For optional source comparisons, set `HY_TEST_HCLI_SOURCE` to the frozen
upstream checkout. Set `HY_TEST_BUNDLE_ORACLE_PYTHON`,
`HY_TEST_RESOLVER_ORACLE_PYTHON`, `HY_TEST_VENV_ORACLE_PYTHON`, or
`HY_TEST_LINT_ORACLE_PYTHON` to the isolated interpreter, selecting only the
corresponding tests. The review used a Python launcher that sets
`HCLI_VERSION=0.0.0` for source imports and preserves each fixture's isolated
HOME; this avoids unrelated configuration-version migration in read-only
adapters. The older AST-based oracles outside this delta were not all migrated
to the new upstream API.

One parallel full-suite run failed a preexisting proxy fixture's connection
classification assertion. The test closes an ephemeral port before expecting
connection refusal and uses a 100 ms deadline, making it sensitive to port
reuse and scheduling. The exact trigger of that failure is unknown. The final
complete serialized run passes; the production proxy code and its test are
unchanged. The manual browser OAuth
fixture remains outside automated execution. Concurrent callback and idle
preconnect behavior is covered by the automated socket fixtures.

## Bounded adjacent findings

| Impact | Finding | Disposition |
|---|---|---|
| High | A failed optional download can leave valid-looking wheels in its destination. | Retry attempts use isolated directories; only the accepted attempt is copied into the bundle. A regression asserts partial files are absent. |
| High | Lexical path validation alone does not prevent writes through an existing destination symlink. | Resolved containment is checked for archive and wheelhouse writes; owned symlink fixtures verify outside files remain unchanged. |
| Medium | Python package versions can change comparison semantics without application changes. | The locked `packaging` 26.0 comparison is executable, including prerelease fallback and arbitrary equality. |
| Medium | Native runtime and filesystem behavior cannot be established by cross-compilation. | Windows/Linux execution, real IDA sessions, live plugin services, and agent registration remain unverified by these local fixtures. |
| Low | Historical parity evidence can be confused with the current upstream range. | This report fixes the endpoint and links the earlier report without claiming its unrelated open boundaries are closed. |

## Quality gates

QG1-QG7 pass within D1-D7: no normative judgment is required; assumptions and
probes are explicit; the named delta has a feature mapping; operation counts
and bounds are reproducible; tests pass with remaining observation limits
identified; provenance is fixed to the inspected source; and adjacent findings
have bounded dispositions. Numeric limits count graph operations and depth;
no physical units or measured performance claims are implied. Native platform,
manual browser, older-oracle migration, and live IDA boundaries remain explicit.

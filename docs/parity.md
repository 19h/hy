# HCLI port review

## Comparison anchors

The upstream checkout is `/Users/int/dev/ida-hcli`, at
`92df5b10c8b2e860a10bbfd8595c8bc2b7c3b64f` (2026-09-14).
The Rust checkout started at `ad22d2eed1ca9ad3a90c7dec1e238cb21b12d438`.
Its package version was 0.18.1. The candidate upstream baseline is tag
`v0.18.1`, commit `2e55915978cbb6176ace24faac5dfc81df4f9fd8`.

The precise upstream revision used for the original port is **unknown**.
The upstream diff from the candidate baseline touches 87 implementation files.
The package version alone does not establish behavioral parity.

## Assumption register

| ID | Assumption | Falsification probe | Dependent conclusions |
|---|---|---|---|
| A1 | Upstream v0.18.1 approximates the last port. | Find a recorded port SHA or behavior introduced after that tag in the original Rust tree. | Attribution of changes to “since the port”; current behavioral comparisons do not depend on A1. |
| A2 | The two local checkout revisions are the requested comparison targets. | Compare `git rev-parse HEAD` with the anchors above. | Scope of this review. |
| A3 | Hy remains a native Rust executable with its own release artifacts. | Inspect project release metadata and the `19h/hy` Git remote. | Self-update targets the Rust repository; Python-runtime-specific HCLI diagnostics need an explicit native equivalent. |
| A4 | Tests must avoid changing the developer's installed IDA, configuration, or protocol registration. | Inspect subprocess environment isolation and filesystem destinations in tests. | Integration tests use temporary homes; macOS compilation tests do not invoke Launch Services. |
| A5 | A legacy shared session must not change the selected credential's account. | Supply a stored or refreshed JWT with a different email claim; both fixtures must fail before the protected API request or configuration write. | Legacy refresh rejects known email mismatches. This claim comparison is not JWT signature verification; tokens without an email claim and live account-email changes remain outside this check. |
| A6 | IDA installation major/minor version components use ASCII decimal digits and fit unsigned 64-bit integers. | Supply metadata with non-ASCII decimal digits or a component above 18,446,744,073,709,551,615; the native ordering reader treats it as unknown, unlike Python's arbitrary-precision integer parser. | Instance version ordering and launch IPC-support detection; normal release fixtures cover 0.0, 9.9 and 9.10, including known-zero versus unknown precedence. |
| A7 | Existing Python extensions run in a selected interpreter containing compatible HCLI/Click packages. | Set HCLI_EXTENSION_PYTHON to a missing interpreter, or expose an entry point without its host package; discovery must fail rather than report an empty catalogue. | Python extension compatibility. The verified runtime contains the pinned upstream checkout as ida-hcli 0.24.0, Python 3.13.15 and Click 8.5.0; arbitrary host versions are not certified. |
| A8 | CPython 3.13.15 supplies the filename-glob, KE query/filename/metadata/IP/cookie and Unix path-resolution oracles for this comparison. | Repeat the corpora under each supported upstream Python version, including missing paths, link cycles, byte encodings, numeric limits, IP registry exceptions and cookie dates. | The 727,831-case character-class digest, twelve KE query cases, fourteen filename cases, seven Unix path comparisons, 13,509 metadata comparisons, a 282,624-case IP digest, 144 cookie policy comparisons and 7,541 expiry-date comparisons. Other Python versions are not certified by these results. |
| A9 | Tested source entry names are representable as UTF-8. | Create a Unix entry with invalid UTF-8 bytes and a wildcard that distinguishes it from the replacement character; also test Windows drive/UNC patterns. | Filesystem glob matching currently converts non-UTF-8 entry names lossily. Arbitrary native-path encoding equivalence remains open. |
| A10 | IPC endpoint names use their process's positive ASCII-decimal PID; Unix PIDs fit signed 32-bit integers. | Supply zero, a negative value, Unicode decimal digits or a value above 2,147,483,647 in a Unix endpoint name. Native discovery ignores unsupported names; Python's integer parser can accept aliases and its kill call can address process groups. | Canonical IDA IPC endpoint discovery. Tests preserve invalid-name entries and probe actual current-process liveness; they do not establish equivalence for forged aliases or PID-reuse races. |
| A11 | KE payload sizes fit native u64 byte accounting. The former ASCII/i64 restriction on setting syntax has been removed. | Model a decoded stream exceeding 18,446,744,073,709,551,615 bytes; compare a larger configured cap with upstream's arbitrary-precision accounting. | Retention values remain arbitrary-precision integers. Positive download caps above the byte-counter range saturate to u64::MAX; this preserves comparisons within that range. Numeric syntax and fallback defaults match 78 upstream cases, subject to A12/A14. |
| A12 | Python integer conversion uses CPython's default 4300-decimal-digit limit. | Change PYTHONINTMAXSTRDIGITS or sys.set_int_max_str_digits, then submit an integer between the two configured limits. | Native metadata validation accepts 4300 digits and rejects 4301; signs are excluded from the count and floats have no digit limit. KE numeric settings, cookie decimal attributes and expiry year/month conversion also use this default bound. Hy does not inherit Python interpreter limit settings. Python's separate runtime-dependent recursion boundary is not reproduced by the iterative native validator. |
| A13 | The upstream HTTP decoder profile is HTTPX 0.28.1 with its default gzip/deflate support and no optional Brotli or Zstandard package. | Install either optional package and repeat a response carrying br/zstd Content-Encoding. HTTPX then decodes it; native handling currently leaves an unknown encoding unchanged. | API and KE share the decoder covered by 31 HTTPX comparisons. Both isolated oracle runtimes report only identity, gzip and deflate in SUPPORTED_DECODERS. Upstream's project dependency is httpx<1 without decoder extras. |
| A14 | Python's decimal-digit classification is Unicode 15.1.0, as supplied by CPython 3.13.15. | Repeat the full code-point digest and integer corpus with a different Unicode database or interpreter integer limit. | Cookie Max-Age, Version and Port parsing and KE numeric settings use the shared arbitrary-precision Unicode parser, subject to A12. The 1,114,112-code-point table comparison covers 680 decimal digits; 6,928 integer inputs, 144 HTTPX cookie cases and 78 environment-setting cases verify grammar and use. Other Unicode versions and extreme local calendar years remain unverified. |
| A15 | Proxy comparison uses HTTPX 0.28.1, HTTPcore 1.0.9 and HTTP/1.1 with UTF-8 environment values. | Repeat proxy selection and TLS-name probes under other runtimes, URL forms and optional proxy transports. Compare installed trust stores and SSL_CERT_DIR behavior. | The 186-case selection oracle and owned HTTP/TLS fixtures establish the tested environment-proxy behavior. Non-UTF-8 environment entries are ignored. Origin contexts use SSL_CERT_FILE replacement roots when configured; HTTPS proxy contexts add the file roots to native webpki roots. Native webpki roots are not certified as identical to upstream certifi/system roots. SSL_CERT_DIR, pooling and optional SOCKS remain open. |
| A16 | OS proxy settings have their documented native types: macOS string hosts and signed 32-bit numeric values; Windows DWORD ProxyEnable and string ProxyServer. | Supply other registry types, out-of-range Core Foundation numbers or settings that change between snapshots. Repeat discovery on native Windows. | macOS uses a read-only SystemConfiguration snapshot and matches the local CPython snapshot. Windows parsing matches 132 CPython cases, but registry access is cross-compiled only. Unexpected registry types fall back to no proxy; CPython can stringify additional value types. Neither snapshot comparison establishes behavior under concurrent OS reconfiguration. |
| A17 | Datetime grammar is compared with CPython 3.13.15 using UTF-8 scalar-value strings; display fixtures use the default English month names. | Repeat the corpus with other Python versions, lone-surrogate separators or a changed LC_TIME locale. Exercise native Windows/local-time clock behavior. | 9,629 ISO inputs compare calendar/time fields, microseconds, naive/aware state and fractional UTC offsets. Forty-five license labels and 21 API-key date pairs use the installed upstream functions with an explicit clock. Other runtime versions, locale changes and non-UTF-8/surrogate values are not certified. |
| A18 | Lint text comparison uses color-disabled upstream Rich output at 10,000 columns and UTF-8 fixture paths. | Repeat with narrow/colored terminals, non-UTF-8 names, alternate path spellings and malformed descriptors that produce multiple Pydantic errors. | Ten valid-source outputs match byte-for-byte under this display setup; nineteen invocations match status. Rich layout, exhaustive path handling and validation-error text remain outside that result. |
| A19 | Background update cache behavior follows the pinned Python implementation, including its cached string-versus-Version comparison failure. Native release-profile execution represents a frozen HCLI binary under A3. | Change the upstream comparison to parse the cached string; repeat with native Windows/Linux cache discovery, concurrent writers, or a runtime using different datetime/SimpleSpec semantics. | Sixteen cache-policy and fourteen release-selection cases compare with the installed upstream functions. A release-profile CLI fixture verifies four local-server scenarios on macOS. Recent cached results suppress checks but do not replay notifications; cross-platform runtime equivalence is not certified. |
| A20 | MCP matching targets the pinned ASCII agent/plugin/marketplace names and headings, with CPython 3.13 casefold behavior. Agent subprocess contracts are tested using owned executable shims and the upstream installer functions. | Change recognized identifiers, Unicode versions, real agent listing schemas or Windows PATH/PATHEXT/shim behavior; submit nonstandard/deep JSON or non-UTF-8 paths. | The matcher includes all eleven non-ASCII scalars folding entirely to ASCII in the reference runtime. Forty-five command sequences match upstream; thirteen CLI scenarios exercise selection and installation ordering. Real agent releases and Windows runtime execution remain unverified. |
| A21 | Shared-file checkbox state follows questionary 2.1.1 with ASCII printable search bindings and both distinct and equal choice values. Terminal fixtures run on macOS. | Supply nonstandard/deep JSON metadata, bracketed paste, non-ASCII search input, narrow/resized terminals or native Windows input. | Two 4,681-sequence corpora compare filtering, selection multiplicity, cursor and no-match behavior. Fourteen terminal scenarios verify selected API keys and cancellation; pagination and null rejection have separate CLI fixtures. Exhaustive terminal/keybinding equivalence remains unverified. |
| A22 | Asset coercion targets upstream's locked Pydantic 2.12.5 / pydantic-core 2.41.5 after CPython JSON decoding. | Run the oracle with another Pydantic version, altered CPython integer limits, nonstandard NaN/Infinity tokens, deeply nested metadata or duplicate JSON keys. | 37,498 integer inputs and 123 Asset inputs match this runtime. A separate isolated runtime avoids the extension environment's Pydantic 2.13.5 / core 2.46.5. JSON numbers retain arbitrary precision; exhaustive decoder equivalence remains open. |
| A23 | Standalone share reports and confirmation grammars target upstream's locked Click 8.1.8, Rich 14.3.2, rich-click 1.9.7 and Pydantic 2.12.5. Source command bodies use in-memory API adapters; native tests use owned local HTTP servers and temporary files. | Use markup-bearing/non-UTF-8 filenames, a different console width, native Windows input, Ctrl-C during canonical input or a live download transport. | Thirteen source invocations compare status and action count; seven successful outputs match byte-for-byte without color at 10,000 columns. Thirty-six input strings compare both confirmation grammars. Terminal signal handling, Rich markup/layout and detailed exception messages remain outside these comparisons. |
| A24 | Upload lifecycle comparisons use the pinned AssetAPI.upload_asset and APIClient.put_file methods with Pydantic 2.12.5, HTTPX 0.28.1, in-memory HTTP transport and a fixed local file. Shared selectors follow questionary 2.1.1 bindings. | Supply other URL forms, redirects, concurrent file mutation, unusual metadata, markup-bearing labels, tiny/resized terminals, native Windows input or changed library versions. | Seventy-five ticket cases compare status and ordered POST/PUT/confirmation events. Eleven upload-selector terminal scenarios cover defaults, navigation, cancellation and conflict ordering; the action selector also verifies Ctrl-Q. Transport, exhaustive terminal rendering and unusual path/Unicode cases remain open. |
| A25 | File-transfer redirects target HTTPX 0.28.1 with an unbuffered async-generator request body. The oracle transport iterates the original stream directly; native fixtures use complete requests captured by owned HTTP servers. | Use early server responses before body exhaustion, inspect partial redirected request headers, supply malformed/non-ASCII Location values, response cookies, other auth schemes, stalled uploads or native Windows transport. | Twenty-two PUT status/Location cases compare success and completed method/path events. Thirty-six native scenarios cover redirects, JSON no-follow behavior, response truncation, forwarded API-key headers and cache HEAD statuses. MockTransport's buffering is explicitly excluded from replay testing. Complete wire, cookie, URL and timeout parity remains open. |
| A26 | API cookie lifetime follows the pinned process-global APIClient; KE retains separately scoped sessions. The shared parser targets CPython 3.13.15 and HTTPX 0.28.1 under the existing cookie assumptions. | Exercise concurrent clients, different account transitions, malformed-cookie cases outside the corpus, custom Cookie headers, alternative interpreters or native Windows networking. | Four integration tests cover seven process invocations for ticket/PUT/confirmation scope, redirects, process isolation, failed responses, expiry and unencodable output. A four-request unit exchange verifies sharing across fresh client objects and clones. The existing 144 policy, 7,394 tokenizer and 7,541 date comparisons pass after extraction. Three source comparisons cover UTF-8, Latin-1 and mixed-header Location decoding. |
| A27 | Shared-file lookup uses the actual pinned APIClient.get_json and error hierarchy, not a mock raising HTTPStatusError. | Replace the underlying client or its status classifier, or raise HTTPStatusError directly from a custom transport. | Six full API-layer source cases establish AuthenticationError for 401/403, NotFoundError for 404, RateLimitError for 429, APIError for 500 and ValidationError for malformed 200 JSON. Twelve CLI cases verify unsuccessful get/delete status and no subsequent transfer/deletion. Exact exception text and custom client substitutions remain outside this result. |
| A28 | API JSON value decoding targets CPython 3.13.15 for standard JSON containing well-formed Unicode scalar values and the default integer digit limit. KE retains surrogatepass syntax validation over disposable text. | Supply unpaired surrogates, NaN/Infinity constants, runtime-specific recursion limits or a modified integer limit. | A 104-case byte/model oracle covers UTF-8/16/32 detection, BOMs, Unicode preservation, malformed byte sequences and global integer limits. Six CLI tests cover 53 scenarios across GET, POST, DELETE, identity and error handling. Compression is separately covered under A13/A29 and message rendering under A30. Unpaired-surrogate values, non-finite constants and exhaustive terminal error rendering remain open. |
| A29 | API response consumption follows the pinned APIClient: buffered JSON and PUT decode before HTTP classification; streamed GET classifies status before body consumption. | Supply corrupt compressed bodies at 401/403/404/429/500, corrupt intermediate redirects, encoded HEAD lengths or optional codecs. Change network chunk boundaries, including the raw-deflate fallback boundary. | Six CLI tests cover 64 process invocations, including 20 error-order comparisons against actual upstream methods with unread HTTPX streams. Decoded bytes/checksums, cache misses from encoded lengths, old-file preservation, request negotiation and upload confirmation ordering are covered. Exact exception text, arbitrary chunk boundaries and optional codecs remain outside this proof. |
| A30 | API error-message conversion targets CPython 3.13.15 str()/repr() over standard JSON values with Unicode 15.1.0 scalar strings. | Supply a different Unicode version, unpaired surrogates, NaN/Infinity constants, deep nesting, arbitrary binary64 bit patterns or Rich markup in a message. | A 203-case oracle checks actual APIClient exception classes/messages. Formatter corpora cover 34,658 numeric and 12,993 nested/string inputs; complete printable classification and single-character repr digests cover every code point/scalar respectively. Two CLI tests cover 32 GET/POST/DELETE/PUT failures. This does not establish Python-only JSON constants, unpaired-surrogate values, recursion limits or terminal markup/prefix equivalence. |
| A31 | Environment creation follows the pinned version/target/tool rules and command lifecycle. Source oracles replace creation/pip services with in-memory adapters; native CLI fixtures use isolated shell executables and owned directories. | Use real uv/venv/ensurepip, native Windows interpreter layouts/PATHEXT, registered IDA runtimes, concurrent target mutation or signals during confirmation. | Thirty-eight source cases cover version precedence/grammar, tool plans and registered interpreter selection; twelve compare complete creation/migration reports after reducing migration error text to presence. Thirteen CLI tests cover 32 invocations, including two PTYs and interpreter disappearance during migration. Actual interpreter installation and exact diagnostic/terminal rendering remain unverified or incomplete. Platform configuration is covered separately under A32. |
| A32 | Platform configuration follows the pinned plan, text-file update and reporting functions. Source file writes are replaced by in-memory byte objects; native integration tests use an owned home and a fake launchctl. | Exercise native Windows/Linux persistence, another locale's subprocess decoding, non-UTF-8 native paths, command termination by signal, concurrent profile changes or a later same-process environment read. | Source comparisons cover 504 plans, 27 session combinations, 152 file updates and eight reporting cases. Five macOS tests cover nine CLI invocations. Native Windows/Linux persistence, exact subprocess/I/O exception text and process-wide environment propagation remain unverified or incomplete. |
| A33 | Doctor findings and classification follow the pinned environment policy. Filesystem predicates are collected before applying policy; binary naming is an explicit policy input. Native fixtures supply shell stand-ins for Python/idat. | Repeat with real IDA, native Windows paths, an unreadable/missing user directory, symlink races, another subprocess locale or general interpreter-resolution cases outside the fixture corpus. | 4,106 policy cases compare complete ordered findings and all twelve pattern categories; seventeen compare filesystem helpers. Seven CLI tests cover 31 invocations, including eight exact plain-text renderer comparisons. The source policy oracle supplies variable-selection and Homebrew predicates; it does not prove their collection. General resolution, exhaustive path/error behavior, Rich styling and live IDA remain open. |
| A34 | Installation and execution share the pinned environment findings but use distinct blocking/probe policies. Dependency metadata is read before environment validation; migration invokes installation directly. | Exercise an unexpected source collection exception, native Windows child execution, a real IDA probe, pip/runtime failures outside the fixtures or interpreter selection outside the configured fixtures. | Ten source formatter cases cover empty/error/warning/mixed findings and custom binary names. Six CLI tests cover 36 invocations across execution, skip-option placement, installation rejection, retained-file preservation and metadata ordering. Existing migration regressions assert one install per plugin without a dry-run. Source exception-driven fail-open behavior remains open; interpreter selection is covered under A37; pip execution is covered separately under A35 and script lookup/execution under A36. |
| A35 | Pip execution follows the pinned helper calls: ordered options, inherited environment/stdin/cwd, captured byte streams and no subprocess timeout. Bundle options are merged explicitly. | Use native Windows handles/path spelling, real package resolution, unusual CLI find-links expansion, cancellation, memory exhaustion or an OS launch error other than the tested disappearance. | 1,536 argument plans and 162 error cases compare actual source helpers with an in-memory subprocess recorder. Two CLI tests cover eleven invocations; a creation regression tests interpreter disappearance between migrations. These prove the tested backend contracts, not real pip installations or complete CLI path/OS-error rendering parity. |
| A36 | Script lookup and execution use the pinned interpreter payloads, with CPython 3.13.15 as the exercised runtime. Native code handles framing, diagnostics, environment construction and child dispatch. | Repeat with native Windows, alternate Python versions, malformed probe objects, noncanonical/non-UTF-8 paths, process-group SIGINT, a timed-out probe or concurrent wrapper replacement. | Source comparisons cover 198 framing/status cases and 48 missing-script messages, and verify both embedded payloads against upstream. Six Unix CLI tests cover 40 invocations when the oracle runtime is supplied, including eight real-interpreter invocations. Interpreter selection has separate coverage under A37; exact outer CLI/OS/timeout errors, exhaustive path conversion and cancellation behavior remain open. |
| A37 | Interpreter derivation receives valid probe observations and follows the pinned candidate layouts and selection precedence. Filesystem comparisons use owned fixtures on macOS with CPython 3.13.15. | Repeat with native Windows, noncanonical/relative/non-UTF-8 paths, inaccessible or concurrently replaced files, malformed/coerced probe fields, real embedded IDA runtimes or cancellation during a shared probe. | 11,520 source derivation cases exercise four filesystem profiles; four in-memory runtime cases compare common probe fields. Four CLI tests cover 23 invocations, including override precedence and cache failure retry. Acquisition has separate coverage under A38; model validation, exhaustive path spelling, native Windows runtime and exact OS error boundaries remain open. |
| A38 | Batch acquisition uses the pinned IDA payload and startup/log contracts; owned shell fixtures substitute for IDA execution. | Exercise real IDA/plugin startup, native Windows, concurrent or inaccessible files, extended attributes, malformed/coerced model fields, non-UTF-8 paths, cancellation or unbounded output. | 668 log cases and one startup-file selection case compare actual source helpers through in-memory write/subprocess adapters. Three CLI tests cover 29 invocations across real-user startup, isolated fallback, log/model failures, child status, environment/stdin/cwd and cleanup. The raw model adapter remains stricter than Pydantic and uses signed 64-bit version fields; complete model/exception parity and live startup behavior remain open. |
| A39 | Explain reports use native runtime identity, valid probe observations and the pinned source note/mismatch/render rules. Plain-text comparisons use Rich at width 10,000 without color. | Exercise HCLI-owned Python environments, native Windows, malformed metadata/probe objects, inaccessible or changing files, narrow terminals, Rich markup in fields or unexpected collector exceptions. | 832 note cases, 144 filesystem-backed mismatch cases and 32 complete text reports compare actual source functions. Six Unix CLI tests cover sixteen invocations across collection order, overrides, nullable fields, versions, PATH candidates and installation provenance. Whole-collector exception/metadata behavior, terminal styling/wrapping and exhaustive path/model conversion remain open. |
| A40 | Plugin find-links values and resolved home paths are representable as UTF-8; lexical behavior is compared with CPython 3.13.15. | Supply non-UTF-8 home/account records, unset HOME with a missing current UID, account-service errors, native Windows environment casing or a different Python version. | 21,120 POSIX/Windows spelling cases and 1,200 Windows home-selection cases compare the actual source callback expression and CPython methods. Six host account cases and CLI empty/relative/unresolved-home cases also pass. Native path encoding and account-service failure equivalence remain open. |
| A41 | Bundle download options and target tags follow the pinned HCLI source with its locked packaging 26.0 dependency. | Use another packaging version, real pip, native Windows process execution, OS launch failures or process-group cancellation. | 2,304 argv plans across all six platforms and 36 byte-error cases compare actual source helpers. Three CLI tests cover five bundle invocations, inherited process state, source order, raw errors and existing-output preservation. These results do not certify real wheel resolution or arbitrary dependency versions. |
| A42 | Bundle target grammar follows CPython 3.13.15 Unicode decimal syntax and its default 4,300-digit integer limit. | Change Python's integer limit or Unicode version, use native Windows or exercise real IDA discovery failures. | 576 grammar/tag/error cases and 1,800 selection cases compare actual source functions. Five CLI tests cover sixteen invocations across Unicode/newline versions, aliases, explicit duplicates, version-only observations and failure precedence. Version subprocess behavior has separate evidence and limits under A43. |
| A43 | Version subprocess text is compared with CPython 3.13.15 using UTF-8 and strict errors. Owned Unix executables provide process evidence. | Run under a non-UTF-8 locale, native Windows subprocess text handling, invalid native paths, process groups or descendants retaining captured pipes; change the source timeout. | 86,279 text cases and 242 version outcomes compare the actual source probe and CPython translator. A native test checks three launch failures, the 10 s deadline and termination of its owned process. Seven CLI tests cover 22 invocations across stdin, decode/status precedence and caller-specific error boundaries. Locale selection, native Windows and descendant/cancellation behavior remain open. |
| A44 | Pip availability follows the source's default 10 s timeout and return-code-only byte-output policy. Owned Unix executables substitute for Python imports. | Use native Windows, unusual executable-path encodings, process descendants retaining pipes or nondefault internal timeout arguments. | 64 source comparisons also run real owned subprocesses, including malformed output and a signal exit. A deadline test verifies three launch failures and termination after timeout. Two CLI tests cover five invocations with inherited stdin/environment and installation guard behavior. Real pip imports, native Windows and descendant cancellation remain open. |
| A45 | Bundle dependency collection follows the pinned source's explicit-list policy. Descriptor comparisons use CPython 3.13.15 and the existing extension runtime's Pydantic 2.13.5. | Repeat descriptor coercion/error cases with locked Pydantic 2.12.5, malformed ZIP encodings/CRC, repository archives with invalid members or all reference grammar cases. | Eleven direct source-helper comparisons and four CLI tests cover inline-only/mixed archives, descriptor suffix/count rules and later installation. Local bundle creation does not apply installation file validation; explicit dependencies retain order/duplicates. Exhaustive descriptor/model/archive equivalence remains open. |
| A46 | Bundle inspection comparisons use ordinary UTF-8 ZIP member names, canonical manifest timestamps and the extension runtime under A45. | Exercise CRC/encryption failures, duplicate ZIP names, unusual path spellings, case-varying plugin identities, numeric/invalid timestamps or native Windows. | Four CLI tests compare fourteen recognition/report outcomes with actual source functions. Descriptor suffixes, invalid-JSON skipping, file-validation early return, continued outer-archive traversal, empty output and failure-output order match those fixtures. Full manifest coercion, display-name selection and unreadable-member handling remain open. |
| A47 | Bundle datetime JSON validation uses speedate 0.17.0 and the numeric parser versions locked by pydantic-core 2.41.5. Source field comparisons run with Pydantic 2.12.5 / CPython 3.13.15. | Change dependency versions, supply nonstandard JSON constants or escaped lone surrogates, exceed JSON numeric limits, introduce simultaneous invalid fields or compare complete Pydantic exception reports. | 32,470 timestamp cases compare normalized values and field-error type/message with the actual source manifest model. Two additional CLI tests compare 31 timestamp/version outcomes using the extension runtime under A45. Version literal coercion, field normalization and failure-before-report behavior match these fixtures. Whole-document JSON syntax, aggregate validation errors and other manifest fields remain open. |
| A48 | Bundle member comparisons use ZIP 8.6.0, CPython 3.13.15 and ordinary central-directory names. Owned fixtures modify specific local headers, CRCs, compression fields and attributes. | Exercise mismatched local/central names, overlapping entries, ZIP64 corruption, duplicate names, malformed encodings, uncommon codecs, interrupted I/O or native Windows execution. | Seven added tests perform 81 source comparisons and CLI invocations: 69 corruption/name/codec reports, six raw local-resolution comparisons and six reports of newly created bundles. Selected bad CRC/header members are skipped; unsupported/encrypted/invalid-DEFLATE members fail. A compound case checks header failure before encryption rejection. Unselected entries are not opened; local creation preserves their bytes. Full ZIP-format equivalence and source exception formatting remain open. |
| A49 | Local bundle inputs use UTF-8 paths and stable home/account and path-existence observations. CLI targets supply distinct sorted platforms. Source comparisons use the CPython runtimes documented under A40/A45. | Change HOME/account resolution or replace/remove a path between platform reads; use non-UTF-8 account records, native Windows or a different Python version. | 21,120 additional lexical cases compare direct pathlib expansion, 128 cases compare the source creation loop's read order and deduplication, and four CLI tests compare 15 local-resolution outcomes. Stable local source behavior matches these fixtures; repeated expansion/stat timing, full repository resolution and OS exception formatting remain open. |
| A50 | Repository bundle comparisons use well-formed snapshot identities, string checksums, three canonical platforms and one input spec. The source repository inventory uses explicit fixtures; selection, fetch verification, creation-loop ordering and descriptor helpers execute upstream code with the A45 runtime. | Omit/null a checksum, supply inconsistent snapshot metadata or broader version/reference syntax, use multiple specs with late descriptor errors, alter repository loading or exercise native Windows/live transport. | Four tests compare 14 source/CLI cases covering eight descriptor shapes, four checksum forms, archive/dependency order and fetch-before-descriptor failure precedence. Fetches repeat per platform; hashes compare case-sensitively; version naming selects the first exact name after all fetches. Complete snapshot validation, selection, repository loading, cross-spec staging order and transport equivalence remain open. Reference parsing has additional coverage under A51. |
| A51 | Plugin reference inputs are representable as UTF-8, and syntax/host matching follows the pinned source patterns with CPython 3.13.15. Lookup parsing is distinct from metadata validation and version matching. | Use unrepresentable argv strings, a different Python Unicode/pattern version, general host URLs outside the closed reference patterns, overflowing semantic versions, malformed snapshot inventories or simultaneous repository-loading errors. | 35,280 syntax cases and 2,000 URL-pattern cases compare parsed values and exact errors. Another 2,400 cases compare bundle preprocessing with a nonlocal-path adapter. Two CLI tests add ten source comparisons for dropped repository scopes, compound specifications and diagnostic examples. Full downstream version validation/errors, general URL normalization, command setup/loading and native Windows behavior remain open. |
| A52 | Snapshot envelope comparisons use standard JSON and the A45 source runtime, with ordinary valid plugin metadata except for explicitly mutated descriptor fields. Version-order fixtures use semantically equal, representable versions. | Use nonstandard JSON constants, duplicate keys, arbitrary metadata coercions, aggregate Pydantic errors, overflowing versions or native Windows. | 180 envelope cases compare acceptance and projected normalized fields; two raw-document cases compare version order. Five CLI tests cover 20 invocations, including 19 source validations/selections, malformed-input rejection before downloads, accepted exports, literal version coercion, installation hash casing and sorted snapshot round trips. Required fields and complete descriptor serialization match these fixtures. Full metadata/JSON equivalence, error reports and broader selection/loading remain open. Snapshot text has separate evidence under A53. |
| A53 | Snapshot text follows the pinned source's default Pydantic JSON serializer and CPython 3.13.15 JSON formatting, with Rust-representable strings and the default integer conversion limit. | Supply lone surrogates, nonstandard input JSON constants, a changed Python integer limit, deep/oversized metadata, untested model coercions or native Windows newline behavior. | One string covers all 1,112,064 Unicode scalar values; 90 layout/number cases compare complete source formatting. Three CLI outputs match the source command byte-for-byte, and two added export/reload selections verify sorted-key effects on version ties. Whole-input acceptance, model coercion, resource/depth limits, error output and native Windows remain open. |

| A54 | Repository selection uses validated snapshots, Rust-representable semantic versions and the A45 source runtime. Installation fixtures contain ordinary valid ZIP descriptors and files. | Probe overflowing versions, additional Unicode casing/URL normalization, malformed later ZIP members, repository loading failures, exact exception rendering or native Windows. | 2,016 identity cases and 5,880 version/location cases compare selected URLs or failure against the actual source repository. Six bundle cases, two invalid-version snapshot cases and nine installation acquisition cases exercise CLI behavior. Source installation comparisons cover acquisition and descriptor selection; they do not execute the entire source installation command. Broader transport, validation, installation and diagnostic equivalence remain open. |

| A55 | Bundle catalogue ordering follows the pinned HCLI implementation running on CPython 3.13.15. Fixtures use valid representable versions, supported compatibility values and standard JSON model fields. | Repeat on other CPython sorting implementations, overflowing versions, nonstandard JSON/NaN, arbitrary model coercions, duplicate ZIP member names or malformed later members. | 8,533 sorting cases, 2,014 catalogue cases and 40 complete bundle CLI/source comparisons establish the represented grouping, ordering, display-name and descriptor-equality behavior. Directory/GitHub catalogue adoption has subsequent evidence under A56. Full metadata validation, transport and installation behavior remain open. |
| A56 | Archive acquisition uses ordinary ZIP encodings, unique member names, valid representable metadata and the A45 source runtime. Filesystem fixtures are stable during each scan; bundle file replacement is tested on Unix. | Supply duplicate raw member names, malformed name encodings, permission/enumeration races, unusual file URLs, in-place archive changes, wheelhouse replacement, other Python runtimes or native Windows. | 66 shared archive-index source cases and eleven CLI/source comparisons cover validation/filter ordering, catalogue snapshots, filesystem aliases, bundle member URLs and member fetching. One Unix unit test verifies retained-file reads and checksum rejection; a GitHub HTTP regression verifies failure propagation. Whole ZIP/model/transport equivalence, repository re-index timing, GitHub archive acquisition ordering and wheelhouse lifetime remain open. |
| A57 | Named ZIP reads target CPython 3.13.15 with default CP437/UTF-8 metadata decoding and no password. Tests execute on macOS; ZIP64 records use small physical fixtures. | Repeat on other Python versions and native Windows, sparse files above 4 GiB, alternate metadata encodings, concurrent file mutation, malformed compressed streams with misleading sizes, and installation/lint/wheelhouse readers. | 1,025 directory/read comparisons, thirteen complete HCLI catalogue comparisons and ten CLI/source comparisons cover the represented duplicate/name/ZIP64/error behavior. The shared reader serves repository indexing, bundle recognition/manifests/fetches and bundle metadata selection. Other consumers, exact exception text, Python warnings, exhaustive decoder behavior and native Windows execution remain open. |
| A58 | Wheelhouse extraction follows the pinned HCLI method and CPython 3.13.15 on Unix. Its source destination is an in-memory file adapter; CRC partial-write fixtures use stored members. | Exercise native filesystem failures, existing destination symlinks, native Windows path/drive rules, compressed-stream read-ahead failures, concurrent in-place mutation and other Python runtimes. | Ninety-three source comparisons cover filtering, path spellings, per-record attributes, duplicate names, output files and represented failures. A CLI regression covers ignored unsafe/corrupt outer members, and the retained-file test now covers wheelhouses. Exact failure text, arbitrary decoder partial writes and native Windows execution remain open. |
| A59 | Installation selection uses the pinned HCLI functions and the A45 runtime; extraction comparisons project the actual source predicates and ZIP reads into an inventory. Fixtures are stable Unix files with represented metadata. | Exercise other metadata/reference grammars, native Windows drive/case rules, local-source replacement between inspection and staging, filesystem publication failures, compressed decoder timing and full source installation with real dependencies. | 325 selection/reference comparisons and ninety extraction projections cover descriptor count/order, lazy named lookup, lexical roots, selected-member validation and resulting bytes. Four CLI regressions cover publication, ignored members, duplicates and Unix permissions. Lint migration follows under A60; complete installation/error/model equivalence remains open. |
| A60 | Lint comparisons target the pinned HCLI archive function and A45 runtime on Unix, using stable ZIP fixtures. Reports compare terminal status, ordered findings and locations while excluding model-specific validation details. | Repeat with arbitrary Pydantic multi-errors, metadata coercion, malformed compressed streams, native Windows path equality, non-UTF-8 filesystem names, concurrent source mutation and Rich markup/terminal rendering. | 181 native CLI/source-function comparisons cover ordered descriptors, duplicate lookup, lexical README parents, reference validation and terminal read/UTF-8 failures. A direct CLI regression checks phase ordering and valid-descriptor accounting. The obsolete scanner and archive inventory are removed; whole lint/model/error equivalence remains open. |
| A61 | Metadata paths are strings from the represented HCLI models. Pure path comparisons use CPython 3.13.15 POSIX/Windows classes; physical directory comparisons run on Unix under A45. | Probe other Python versions, native Windows filesystem behavior, non-UTF-8 roots/surrogates, arbitrary stat failures, concurrent replacement, platform-specific special files and full regular-directory packaging. | 3,329 grammar cases, 86,554 lexical joins, 196 directory/reference/dependency cases and an effective permission-denial probe match source behavior. Two CLI regressions cover four literal/normalized names and four directory-reference forms. Windows error mappings are source-reviewed and cross-compiled; native runtime equivalence remains open. |
| A62 | Regular-directory packing comparisons use the pinned HCLI function, CPython 3.13.15 and stable Unix fixture trees under A45. The compared ZIP projection includes member order, decoded bytes, sizes, DOS timestamps, compression method and ordinary rwx permissions. | Probe native Windows traversal/case ties, non-UTF-8 names, alternate timezones/runtimes, ZIP64 size boundaries, special permission bits, concurrent mutation during acquisition and arbitrary filesystem failures. | Twelve source packing comparisons, seven new CLI regressions and a retained-source test cover the represented distribution behavior. ZIP bytes and complete external attributes are not identical; special mode bits are discarded by the native writer and are not restored by installation. Source replacement after acquisition cannot change regular installation bytes. Whole installation/model/transport equivalence remains open. |
| A63 | Direct installation is classified using the pinned command's branch order and A45 runtime. GitHub parsing is compared only after the source direct-install pattern accepts an input. Physical path fixtures run on Unix. | Probe native Windows filesystem encodings/errors, concurrent path replacement, other Python regex runtimes, downstream URL serialization/transport, malformed release responses and complete command diagnostics. | 1,822 source-branch projections and 25,088 actual GitHub recognition/parsing comparisons pass. Four CLI regressions cover local suffixes, directory/archive precedence, rejected direct schemes, file URLs and observed GitHub release requests. The parser preserves source owner/repository/raw-tag spelling; full transport and native Windows runtime equivalence remain open. |
| A64 | File-URL conversion uses CPython 3.13.15, Unicode 15.1 and the A45 Unix runtime with UTF-8/surrogateescape filesystem encoding. Windows conversion is compared through nturl2path and PureWindowsPath on that runtime. | Probe other filesystem encodings, lone-surrogate input strings, native Windows file operations, arbitrary IPv6/IPvFuture spellings, Unicode normalization contexts, filesystem-specific names and concurrent replacement. | 5,315 conversion comparisons, the complete non-ASCII NFKC-delimiter table, 45 archive reads, six repository-construction comparisons and a direct-install regression cover represented file-URL behavior. This macOS volume rejects invalid UTF-8 filenames; its read failure is compared, while decoding bytes are tested independently. Full HTTP transport and native Windows execution remain open. |
| A65 | Repository fetch policy is compared with the pinned source and HTTPX in the A45 runtime. The source client uses an in-memory transport; native wire regressions use owned HTTP loopback servers. Authentication resolution is replaced with deterministic empty or API-key headers in the policy oracle. | Probe real authentication refresh, arbitrary HTTPX URL normalization/joining, proxies/TLS, duplicate headers, malformed wire responses, timeout phases, optional content codecs and native Windows networking. | 231 response-sequence comparisons and 280 raw-host decisions pass. Three CLI regressions cover 37 wire scenarios. Lazy credential reuse, exact redirect statuses, represented cookie transitions, decoding order and entitlement diagnostics agree within this corpus. Generic HTTP diagnostics, URL serialization and phase deadlines remain distinct or unverified. |
| A66 | Direct GitHub release acquisition uses the pinned source and A45 runtime. Ordinary JSON values and represented UTF byte encodings are compared through the actual source fetch function with intercepted GET operations. Redirect policy uses actual HTTPX client handling over MockTransport. | Probe Python JSON nonfinite constants, lone surrogates, nesting limits, malformed/normalized URL forms, live TLS/proxy behavior, phase deadlines, optional codecs and native Windows execution. | 1,998 selection comparisons and 125 HTTP transition comparisons pass. Three added CLI regressions cover eleven owned-server scenarios. Asset defaults, delayed field access, numeric size decisions, selection diagnostics, request headers, separate cookies and represented redirect/error order agree. General JSON and HTTPX URL equivalence remain open. |
| A67 | Release JSON uses CPython 3.13.15, Unicode 15.1 and its default 4,300-digit integer limit as the decoding oracle. Strings retain Unicode code points, including surrogates. Policy diagnostics project the runtime's UTF-8 stderr/backslashreplace behavior. | Probe other Python/Unicode runtimes, configured integer limits, nondefault stderr encodings, exact JSON exception diagnostics and resource-failure boundaries. Audit other JSON consumers independently before adopting the reader. | 2,012 decoder comparisons, complete Unicode scalar scans for ZIP-letter lowercase mappings, 2,196 release-selection comparisons and six added CLI scenarios pass. Native parsing and destruction are iterative. A separate source probe accepts depth 5,000 but rejects 10,000; the native stability fixture accepts 10,000, so runtime resource-limit parity is not established. |
| A68 | Automatic redirect construction targets HTTPX 0.28.1 under A45. Comparisons use representable HTTP/HTTPS URLs and normalize only an empty source URL path to its transmitted `/` form. Repository manual redirects retain their separate contract. | Probe percent-encoded hosts/dot segments, backslashes, unusual authorities and ports, non-HTTP schemes, complete URL/error serialization, live proxies/TLS and native Windows. | 480 source target comparisons, 133 expanded GitHub request-policy comparisons and four added owned-server CLI scenarios cover missing-host repair, duplicate Location headers, literal dot segments, fragment inheritance, credentials and Host headers. Complete HTTPX URL and raw-wire equivalence remain open. |
| A69 | Catalogue retry comparisons use the pinned HCLI functions, CPython 3.13.15 and locked Tenacity 9.1.4 under A45. Time and urllib acquisition are intercepted in memory; native wire fixtures use stable owned loopback endpoints. Python's socket default timeout is unset. | Probe other runtimes, global socket timeout overrides, socket write failures, resets/TLS/proxies, redirect/error presentation, native Windows, real GitHub quotas and cancellation. | 562 actual decorated-function sequences cover nested counters, status/error classification, header precedence and reactive/proactive waits. A paused-clock refused-connection regression checks the production scheduler. CLI fixtures cover retries in all remote catalogue consumers, request replay, cache publication, terminal failures and raw archive bytes. Full catalogue transport/discovery equivalence remains open. |
| A70 | Archive planning uses valid represented GitHub metadata and the pinned `GithubPluginRepo.get_plugins` collection logic under A45. Repositories are unique, metadata strings are Unicode scalars and sizes fit unsigned 64-bit integers. CLI fixtures use stable owned files and HTTP endpoints. | Probe full Pydantic coercion/malformed models, negative/huge sizes, URL-related ValueError mappings, upstream cache-path interoperability, filesystem name aliases, concurrent cache changes and native Windows. | 550 source collection projections and four added CLI tests cover global phases, tuple sorting, duplicate retention, tag URL deduplication, logical cache identities, cache-before-size ordering and represented acquisition failures. Cache storage remains native and account/origin partitioned; full catalogue/model/transport parity remains open. |
| A71 | GraphQL batching uses unique normalized ASCII repository identifiers, represented JSON/model values and stable native cache files under A45/A70. | Probe non-ASCII or quoted identifiers, full Pydantic coercion, nonstandard JSON, corrupt/unreadable caches, cache-write failures and concurrent filesystem changes. | 113 upstream query/envelope projections and three CLI tests cover ten-miss batches, partial results, lookup order and query/model failure publication. Cache-write transactions, upstream cache interoperability and complete model/transport equivalence are not established. |
| A72 | Catalogue cache reads use stable owned filesystem paths and CPython 3.13's existence policy; metadata times are representable binary64 epoch seconds. | Probe concurrent replacement, permission changes, sub-microsecond expiry boundaries, alternate filesystems, malformed model coercion and native Windows execution. | 33 macOS source-getter comparisons and four CLI tests cover represented hit/miss/error outcomes, strict expiry, future dates, deletion-before-refresh and propagation of read/decoding failures. Source deletion is intercepted; Rust performs the actual fixture mutations. Cache layout, write semantics, full JSON/model grammar and race equivalence remain open. |

## Implemented contracts and remaining coverage

“Implemented” below identifies code present in this working tree. It does not
imply that every upstream edge case or supported operating system was tested.

| Area | Implementation and evidence | Remaining coverage |
|---|---|---|
| Formatting and module structure | Repository rustfmt configuration; separate plugin configuration, installation/publication, canonical installed records, older-format inventory, editable registration, bundle commands, repository transport/configuration/reference parsing, Python environments, link download/navigation, shared asset uploads, share list management, license API/download/display/installation, download navigation/transport, metadata-preserving file-copy modules, CLI parsing/status, local installation version readers, platform discovery and instance registration/listing. Instance rows and discovery candidates use named fields. The `ida` and legacy `ke ida` aliases dispatch to the same implementation. | Continued review is required as parity work changes these modules. |
| Global configuration | Fallible writes stage the complete JSON file and update memory only after persistence succeeds. Related insertions/removals can commit together. IDA removal, automatic registration and protocol setup commit instance/default changes together; manual registration preserves the default. String mappings preserve insertion order, including source search precedence. Legacy nested IDA/source keys are removed during migration so later removals cannot revive stale values. The obsolete set_nested write alias was removed. | Concurrent-process writes remain last-writer-wins. Read-only commands defer migration persistence until a later change; upstream writes migrations during initialization. Switching updates the CLI default before idalib configuration, preserving upstream's possible partial success across two files. |
| Credentials and authentication persistence | Timestamp fields preserve upstream strings, including its `+00:00Z` suffix; credential mappings retain insertion order and default reassignment. Malformed records produce actionable errors. Credential/session/login-email changes commit together; failed writes retain prior memory. Login, validation and refresh HTTP work run outside Tokio's async workers. API/repository header construction is shared and fallible. `auth key install --key-name` is supported and validation failures return nonzero. Removing interactive credentials clears associated legacy session data in the same commit. | Current upstream uses a lightweight GoTrue client and does not persist the old SDK session key. Hy retains legacy refresh as a compatibility extension. All credential-model coercions remain open. `--name` remains a Rust fallback when `--key-name` is absent; upstream currently ignores that option in key installation. |
| Authentication runtime and constraints | Stored interactive tokens are validated through GoTrue `/user` before authenticated requests. Opaque tokens use the server's verdict; legacy refresh occurs only after rejection and is revalidated before persistence. Empty tokens do not authenticate. Invalid interactive tokens become anonymous for optional repositories and appear logged out in `whoami`. Commands corresponding to upstream AuthCommand enforce `--auth` and named-credential constraints before file access, prompts or API side effects. Environment keys retain precedence; a forced managed name fails when an environment key leaves no current managed credential, as upstream does. | Live service validation and exhaustive response coercions remain open. Validation is lazy rather than issuing network requests during every local auth initialization. Non-string user emails are rejected. Error precedence differs when credentials and forced-type options are both invalid. |
| Authentication status | `whoami` resolves environment-key identity through `/api/whoami`, with upstream's api-key-user fallback on lookup failure. `auth default` commits its selection before the same lookup; its local selection logic occupies a separate module. Network identity lookup runs after releasing the auth mutex. Async login/switch/key-install status retains the placeholder. Standalone identity requests send the supplied key, do not follow redirects, and use 60 s connect/read inactivity deadlines. Six CLI tests cover twelve response/error cases, redirect JSON and fallback, invalid headers, managed status, default-selection ordering and async installation. Managed status leaves credentials and last_used unchanged. | Live accounts, exhaustive transport/certificate behavior and terminal rendering remain unverified. Identity lookup failure does not invalidate an environment key for status: upstream treats its nonempty presence as logged in. Empty string email fields remain valid API model values. |
| Datetime parsing and reports | A shared parser separates calendar/week dates from time/offset grammar and retains naive versus aware values. It supports basic/extended dates, week dates, arbitrary single-character separators, truncated microsecond fractions and fractional-second offsets. Command-specific formatters replace the former generic date display: credential columns share their fallback boundary, API keys use month/day/year and relative usage, shared files retain seconds, and license expiration has explicit-clock tests. Five unit tests include 9,629 CPython inputs, 45 upstream expiration labels and 21 key-date pairs; three CLI fixtures cover all four report consumers. | Depends on A17. Exhaustive malformed-input equivalence, live clock transitions, runtime-specific locale behavior and native Windows rendering remain unverified. License date fields remain strings and sort lexically; naive values retain raw expiration text. Numeric API date fields are still rejected by the upstream string model. |
| Login and logout | OAuth URLs encode query parameters and include `prompt=login` for forced account selection. OAuth and OTP credentials require remote user validation before persistence; OAuth no longer invents email or expiration fields from an unverified token. Unix PTY tests exercise OTP success/rejection, forced OTP sign-out, named/all logout confirmation, cancellation and failed persistence. All-credential removal commits once; associated legacy session cleanup commits with credential removal. | Live OAuth browser/account round-trip, multi-credential login/default-selection interaction, multi-account legacy-session cleanup and Windows terminal behavior remain unverified. Forced OTP explicitly attempts remote logout for the selected stored interactive token; upstream's separate client/session objects can omit that request. Legacy session removal is an extension because current upstream does not persist it. |
| OAuth callback transport | The loopback listener binds before browser launch. Hyper handles HTTP/1 framing, fragmented headers/bodies and chunked requests. Bounded concurrent connections tolerate idle browser preconnections. Successful responses finish before callback shutdown. Socket regressions cover 12,000-byte tokens, malformed submissions, exact token routes, callback query parameters, body limits, occupied ports and deadline cleanup. The callback page is a separate HTML artifact and submits to its own origin. | Live browser execution is not verified: automation reported no browser surface, and Safari/Chrome selection failed with `cgWindowNotFound`. A manual browser fixture remains explicitly ignored in the automated suite. Token JSON is limited to 64 KiB and non-string token fields are rejected; malformed JSON returns 400 rather than upstream's 500. Chunked requests are accepted beyond upstream's Content-Length-only handler. |
| Plugin manifest | Required versioned wrapper, entry point, repository identity and contacts; canonical list defaults; nullable descriptions; category/platform/version constraints; preserved extra plugin metadata. Validation runs at the metadata deserialization boundary for archives, directories, installed descriptors and snapshots. Required fields use non-optional types. A pinned upstream oracle covers 58 validation cases. | Python regex-engine differences and numeric/lexical edge cases outside the current oracle remain open. Referenced-file validation has its own oracle and CLI regressions below. |
| Plugin files and lint | Shared ASCII/relative-path and logo checks; ZIP native entry points require binaries for every declared platform, while directories preserve upstream's more permissive rule. Repository, installation and lint references use exact directory names under A59/A60. Lint uses ordered named reads, separates descriptor discovery from reference validation and compares lexical parents for README discovery. Inspection and recommendation reporting occupy separate modules. Findings use stdout and successful status; input/transport/I/O and UTF-8 decoding failures remain command errors. Lint does not resolve dependency files. Current-user tilde expansion, canonical existing paths and directory README file symlinks are supported. The file-validation oracle covers 38 cases in two archive layouts and a directory; 19 lint invocations compare upstream status, with exact plain-text output compared for ten valid sources. A60 adds 181 archive CLI/source comparisons and a phase-order regression. | Explicit native filenames in ZIPs retain upstream's rejection quirk. Installation rejects symlinks during selected-subtree extraction; lint's reference lookup does not inspect attributes or member contents. Pydantic multi-error counts/details, exact reference-error text, Rich terminal styling, other-user home expansion and exhaustive archive/path normalization remain open. HTTP URLs remain a native extension; upstream recognizes HTTPS only. |
| Plugin versions | Dedicated semantic-version coercion and SimpleSpec matching; prerelease bounds, partial versions, service markers and build-sensitive identity. Precedence comparisons ignore build metadata. A pinned upstream dependency oracle covers 30 input versions and 35 specifications. | Numeric components beyond Rust's `u64` range, Unicode coercion and exhaustive reference grammar remain outside the verified domain. |
| Plugin compatibility | Defaults and allowed values come from the pinned schema: 63 IDA versions and six platforms. IDA ranges expand to exact known versions, including service packs; empty lists match nothing. Platform selection reads the selected IDA executable and honors `HCLI_CURRENT_IDA_PLATFORM`. | Real cross-architecture IDA execution and all repository archive variants remain unverified. |
| Plugin schema | `schemas/ida-plugin.json` is copied from the pinned upstream `docs/schemas/ida-plugin.json`. Flat manifests, missing wrapper versions and dictionary-form settings now fail as they do upstream. | Schema equivalence does not establish runtime model equivalence; Pydantic's accepted boolean coercions and IDA range expansion are tested separately. |
| Plugin installation | Validate and stage files before pip; resolve combined dependencies with pip dry-run, excluding the replaced version; publish after dependency installation succeeds. Direct archives count all valid descriptors; named lookup stops at the first exact name under A59. Reference validation is separate from selection. Extraction validates only the selected subtree, excludes its raw `.git/` prefix, rejects selected symlinks and reads duplicate names through their last record. Replacement directories use the exact new descriptor name, including case. | Archive normalization and malformed-member cases beyond A59, full source installation equivalence and cross-platform publication failures. Pip environment changes are not rolled back after a subsequent failure. |
| Local directory distribution | Under A62, regular directories are packed before metadata inspection and use the shared archive pipeline. Packing filters, file-link dereferencing, omitted directory links/empty directories, all-descriptor counting and fresh installed-file permissions are covered by source comparisons and CLI regressions. `.venv`, `venv` and `.idea` files are retained. A single archive snapshot survives inspection through staging. A63 preserves directory/archive classification through acquisition. | Native Windows execution, non-UTF-8 names, arbitrary special files, ZIP64 boundaries, concurrent acquisition and complete ZIP metadata/byte identity remain unverified or differ as recorded under A62. |
| Direct installation source selection | A63 uses upstream branch order for editable directories, regular plugin directories, existing lowercase `.zip` paths, `file://`, recognized GitHub URLs, lowercase `https://`, and repository references. GitHub owner/repository/tag parsing preserves source spelling after recognition. | Full command diagnostic text, native Windows path behavior, URL decoding/serialization, transport policy and malformed release-response handling remain open. |
| File-URL acquisition | A64 shares source-compatible decoding between archive fetching and repository construction. Authority validation is separate from local path selection; semicolons, encoded delimiters, trailing spaces, symlink/parent traversal and Unix filesystem bytes are retained under source rules. | Non-UTF-8 filesystem encodings, lone-surrogate input strings, native Windows execution and arbitrary filesystem/authority edge cases remain unverified. |
| Repository HTTP policy | A65 resolves credentials lazily once per fetch, attaches them only to eligible raw HTTPS hosts, follows the five source redirect statuses with Location, retains per-fetch cookies, decodes bodies before status handling and distinguishes missing credentials, rejected credentials and entitlement denial. | HTTPX URL serialization/joining, generic HTTP diagnostic text, proxy/TLS and timeout-phase behavior, live credentials and native Windows networking remain open. Direct GitHub release transport has a separate source contract. |
| Direct GitHub release acquisition | A66 selects ZIP assets after inspecting names, reads size/download fields only for the sole candidate, applies the source 104,857,600-byte metadata limit, and supplies separate metadata/asset HTTP operations with source Accept values and 30 s/60 s connect/read settings. Twenty redirects are permitted; final HTTP errors precede final-scheme downgrade checks. A67 accepts source nonfinite numbers, preserves surrogate strings and handles represented deep metadata. | Exact Python resource-failure boundaries, arbitrary HTTPX URL construction, generic error presentation, write/pool timeout semantics, TLS/proxies and native Windows execution remain open. |
| Python JSON values in release metadata | A67 supplies a typed reader for null, booleans, arbitrary integers, binary64 floats, Python strings, arrays and ordered objects. It preserves nonfinite numbers, surrogate code points, signed floating zero and duplicate-key replacement order. UTF-8/16/32 byte decoding shares source encoding detection. | Adoption by other API/model/configuration JSON consumers is not implied. Full malformed-input diagnostic text, custom Python runtime limits and exact recursion/memory-failure behavior remain open. |
| Automatic redirect targets | A68 shares target construction between streamed API transfers and GitHub acquisition. Duplicate Location values are combined using response-wide text decoding. Absolute targets without a host inherit only the previous host; literal dot segments are normalized before that repair. Control characters and oversized Locations are rejected before consuming the redirect body. | WHATWG URL normalization still differs from HTTPX for encoded hosts/path segments and other URL forms. Full diagnostic and serialized-URL equality, non-HTTP schemes and native Windows execution remain open. |
| Editable package registration | `src` layouts write `_hcli_editable_NAME.pth` into the selected IDA interpreter's `sysconfig` purelib directory. Flat/regular replacements remove stale registrations. Uninstall removes links and registrations while retaining source files. Broken entries can be replaced or removed. Staged registration errors preserve the old plugin; cleanup skips interpreter-discovery failures, matching upstream. A real isolated Python process imports the fixture and observes later source edits. | Windows symlink/registration execution and live IDA imports remain unverified. Registrations left in a previously selected interpreter, and stale filenames after name-case changes on case-sensitive filesystems, remain open. |
| Installed plugin inventory | Managed operations share records whose descriptors, referenced files and exact directory names validate. Broken directories do not enter dependency preflight, search, upgrades or configuration. Unfiltered status separately lists minimal descriptors and single-file legacy plugins; named status accepts only managed records and retains requested order/repetitions. Broken entries remain removable by filesystem name, including UTF-8 legacy names. | Directory entries are sorted for deterministic reports rather than retaining upstream filesystem iteration order. Case-colliding installed names produce errors in destructive/metadata lookup paths; upstream may select its first record. Non-UTF-8 filenames and exhaustive Unicode case conversion remain unverified. |
| Python dependency metadata | Installation, dependency preflight and migration resolve explicit requirements or PEP 723 inline metadata. Lint does not resolve dependency scripts. Bundle creation collects explicit lists only and leaves inline metadata untouched, matching its source-specific policy. Tests cover archives, directories, editable sources, retained neighbors, malformed scripts and later installation from an inline-only bundle. Plugin code is not executed to extract requirements. | Depends on A45 for bundle collection. Migration and real wheel resolution still need end-to-end verification. Non-string TOML dependency entries are rejected during parsing; upstream returns them and fails downstream. |
| Plugin upgrades | Search all repositories by installed identity; explicit repository scope; require a newer version for `plugin upgrade`; preserve equal/newer installations for `plugin install -U`; replace editable installs. Status and upgrade both reject ambiguous duplicate repository identities; status names are case-insensitive. | Full version grammar and cross-platform case-only replacements. |
| Plugin configuration | Upstream `plugin config PLUGIN` interface, including setup; descriptor defaults resolved on read; canonical installed names; typed setting kinds; ordered descriptors; secret redaction; atomic multi-setting updates. Argument parsing and interactive forms occupy separate modules. Nine terminal fixtures cover editable defaults, boolean/choice fallbacks, hidden secret input and validation retries, choice preflight, deferred optional-blank validation, cancellation, and fresh-install rollback versus retained upgrades. Repeated install arguments parse types immediately and validate the final value per key. An empty import argument reads stdin. Twenty-five regex cases cover prefix matching, final-newline anchors, scoped flags, comments, lookarounds and backreferences against CPython. | Exact rendering and questionary key bindings differ; Ctrl-U line clearing is not provided by the native prompt library. Native Windows prompts and complete Python regex syntax/Unicode equivalence remain unverified. Atomic invalid-import/setup behavior deliberately differs from upstream incremental writes, as described below. |
| Named repositories | Reserved defaults, legacy repository interpretation, recursive local directories, multiple plugins per archive, JSON snapshots, bundles, scoped references, credential-scoped redirects and hash verification. Named custom repositories filter claims to Hex-Rays identities. Installation repository overrides reject incompatible named prefixes; bundle preprocessing drops parsed prefixes under A51. Typed name/range/exact search reports and multi-name status. | Full descriptor/archive validation, downstream version matching and loading/error precedence; precise ordering across archive variants; ordinary snapshot cache behavior. |
| Plugin reference syntax | Shared parsing separates lookup names, unvalidated version text, supported host qualifiers and repository scopes. It rejects direct GitHub install URLs before prefix parsing, peels the scope before the last host separator and preserves source error precedence. Host matching follows Python case folding and terminal-LF rules; supported identities preserve dot components and Unicode spelling during normalization. | Depends on A51. Parsed names are lookup strings, not validated metadata. General URL normalization outside the supported identity patterns, version matching and command-specific loading/error behavior remain separate limitations. |
| Repository snapshot envelopes | Dedicated wire models require the plugin list, plugin host, archive URL/hash and complete versioned descriptor. Snapshot version defaults to literal 1; shared schema-version decoding accepts source literal coercions. Location descriptors reuse the local manifest model and serialize their required version while excluding $schema. Version maps preserve input document order, including equal-precedence selection. Exports sort keys, indent by four spaces, escape non-ASCII strings and apply source number formatting. Bundle and installation downloads share case-sensitive hash verification. | Depends on A52/A53. Full metadata coercion, duplicate-key/nonstandard JSON behavior, exact validation reports, repository initialization and broader version-selection behavior remain open. Sorting exported keys can change a version tie after re-import, as upstream does. |
| Pip offline mode | Group `--offline` changes pip's index policy without disabling repository transport. It requires `--pip-find-links` or an explicit bundle repository, except for upstream's repository-free command groups. A local HTTP fixture verifies repository/archive fetches while pip receives `--no-index`. | Full upstream pip environment classification remains open. The existing Rust search `--offline` and status alias remain separate command options. |
| Plugin bundles | Separate target, manifest, inspection, publication, download and CLI source-resolution modules. Repository packaging verifies hashes, fetches per platform, groups archives in first-seen order and names them from the first exact-name descriptor. Installation uses archive metadata under A54. All archive repositories share catalogue grouping and ordering under A55/A56; bundle locations retain member URLs and fetch through an owned reader. A57 preserves duplicate name order and last-member lookup; A58 integrates wheelhouse extraction and A59 integrates installation reads. Selected Python, inherited pip sources and 30 platform/Python targets are supported. macOS tag sequences match packaging 26.0 from the upstream lockfile. Downloads use their own source-option order, omit installation-only flags, inherit stdin/environment/cwd and preserve raw decoded failure streams. Bundle consumption flattens wheel files, rejects duplicate basenames, checks target availability even without dependencies, and respects custom sources. | Depends on A41–A50/A54–A59. Real pip wheel resolution, all multi-plugin/native archive combinations, whole-document JSON/model-error equivalence, broader acquisition/loading/selection/reference grammar, compressed-stream partial-write behavior and cross-spec staging order remain open. Tests use fixture wheels and interpreters. |
| GitHub catalogue | `--repo github`, extra/ignored repository lists, code-search discovery including forks, GraphQL release/tag metadata, distribution and source archives, date/type/size filtering, source identity checks, and account/origin-partitioned caches. Metadata expires after 86,400 seconds; archive bytes persist. A69 adds nested retries and proactive waits. A70 collects metadata before acquisition, orders global asset/source phases, preserves repeated release entries, deduplicates tag URLs within a repository and uses release/name or commit cache identities. Cache lookup precedes the asset download-size check. A71 warms metadata in ten-repository GraphQL batches, preserves partial NOT_FOUND results and validates each complete batch before cache publication. A72 propagates cache read/decoding failures, accepts future-dated entries and removes expired metadata before refresh. | Complete discovery/model coercion, upstream cache paths/formats, write semantics and filesystem aliases/races, urllib redirect semantics, additional ValueError/network-error mappings, HTTP failure diagnostics, live private repositories and native Windows remain unverified or incomplete. |
| Python commands | Exec/script argument passthrough; separate typed doctor and explain reports; environment creation, dependency migration and persistent environment variable configuration. Explicit pip source/offline/build-isolation options and environment-check override. The Python group accepts --no-python-environment-check before the leaf; identically named arguments after exec remain child arguments. | Explain collector edge cases, IDA probe model validation, exhaustive path conversion, subprocess environment/signal behavior and cross-platform configuration edge cases remain open. Creation-specific discovery, doctor policy and execution/install guards and explain reports are covered separately below. Hy reports its own Python interpreter as not applicable because it is native Rust. |
| Python explain report | Separate report records, installation/runtime collectors, ordered notes and complete text rendering. Overrides preserve their absent IDA probe; version collection resolves independently and uses the bounded version helper. Embedded virtualenv details come from the probe's VIRTUAL_ENV. Mismatch checks cover activated/requested roots before the final interpreter, deduplicating normalized roots. PATH candidates preserve order, deduplicate resolved aliases and exclude uv overlays. Known-installation versions use SDK/directory metadata, while selected-version reporting retains override/registry/SDK/binary/directory provenance. | Depends on A39 and acquisition/model limits under A38. The tested observations and source text rules match; full collector error handling, discovery/metadata equivalence, native Windows, noncanonical paths and terminal Rich behavior remain unverified. Native runtime identity is explicit; no fictitious HCLI Python version or own-venv branch is supplied. |
| Python doctor | Separate state collection, filesystem observations, ordered findings, setup patterns, context notes and rendering. Version and pip are independent 10 s observations shared with creation; pip remains unknown when the resolution probe marks a managed base interpreter. Explicit HCLI overrides bypass additional IDA probing. All ten checker finding IDs and twelve pattern categories follow source precedence, including early return for a missing override interpreter and a separate unresolved-Python report. Findings retain complete details and concrete hints; arbitrary minimum-version and installation-platform findings were removed. Text groups errors before warnings and includes setup, fixes and context notes. | Depends on A33. The collector uses the selection policy covered under A37 and native installation metadata helpers. Native paths, missing-user-directory handling, live IDA execution, subprocess decoding and Rich wrapping/styling are not fully certified. A native executable has no sys.prefix environment to exclude from shell-venv discovery. |
| Python environment guards | Shared findings drive installation validation and advisory exec/run-script/find-script checks. Installation can probe IDA when variable-based resolution omitted it, prints all findings, rejects errors and independently requires pip even when diagnostics are skipped. Warning-only execution never adds an IDA probe, preserves child exit status and continues through error findings. The group skip flag suppresses those observations but leaves explicit doctor behavior intact. Combined dependency metadata is parsed before interpreter checks; rejection precedes pip and publication. Separate pip resolution and install functions retain the install preflight while migration runs only the install operation. | Depends on A33/A34. Native collection is best-effort but does not model every exception caught by the source wrappers. Exact outer CLI errors and native Windows execution remain open. Script lookup/execution and pip transport have separate coverage below. The source's error-labelled advisory text is retained even when execution continues. |
| Pip execution | Separate command construction and byte-error rendering. Default operations add only source options; bundle merging enables isolated/no-cache/version-check flags and prepends its wheelhouse. Empty index_url suppresses its argument but still counts as a custom source. Requirements remain separate argv entries without an injected -- delimiter. Pip inherits environment, stdin and cwd; stdout/stderr are captured and waits have no fixed timeout. Nonzero status decodes both streams as UTF-8 with replacement, strips Python whitespace and joins stdout before stderr. PEP 668 recognition precedes the old-pip dry-run error. Preflight wraps package errors with the proposed dependencies; install and migration retain raw reasons. Migration records package failures but propagates OS launch failures. | Depends on A35. Real pip resolution, native Windows runtime, process cancellation and exhaustive OS-error formatting remain open. Output buffering is unbounded. Pip's inherited environment intentionally differs from the environment constructed for exec/script commands. |
| Pip source paths | The plugin callback preserves strings containing :// and applies shared lexical pathlib normalization plus home expansion to other find-links values before command dispatch. Relative paths and parent components remain relative; empty input becomes a dot. POSIX double-root spelling, Windows drive/UNC/device paths and platform-specific home selection have separate tested policies. | Depends on A40. Bundle sources use the shared implementation under A49, without the URL exception. Other commands, native Windows execution, arbitrary path encodings and account-service failures remain open. |
| Local bundle sources | Path expansion precedes existence-based local ZIP selection. Directories ending in .zip reach the file-read error, and descriptor diagnostics retain the original spec. Literal existing inputs are read once; inputs found only after home expansion are read per distinct target platform. Equal bytes deduplicate in first-seen order, with a platform suffix only when contents differ. | Depends on A49. Native code observes expansion/existence once before platform reads; source helpers repeat those observations. Concurrent filesystem/account changes, repository reference grammar/loading/fetch order and complete exception rendering remain open. |
| Bundle target selection | Target grammar and CLI selection are separate. Versions accept source Unicode decimal digits, a final LF and arbitrary-precision values within CPython's default conversion limit. Ordinary platform options canonicalize aliases; explicit target IDs and current-platform observations preserve their spelling and fail during tag lookup if noncanonical. Explicit target duplicates remain; platform/Python products deduplicate by target ID in first-seen order. Missing-option errors and runtime observation order match the source. Current Python uses the independent version-only helper. | Depends on A42. Runtime configuration outside the pinned Python/packaging defaults, shared version-probe subprocess details, full CLI styling and complete bundle manifest validation remain open. A bundle created with duplicate explicit IDs is rejected by the source and native manifest readers. |
| Version subprocess observations | Separate command, result policy and strict text translation. The probe inherits stdin/environment/cwd, captures both streams and has a 10 s deadline. UTF-8 decoding and universal-newline conversion precede status checks. Launch failures, timeout, nonzero status with decodable output and empty stripped stdout return no version; malformed text propagates as a decoding error. Doctor, creation and current-version detection propagate; guards skip diagnostics while retaining mandatory pip validation; explain captures mismatch-collection errors. | Depends on A43. Non-UTF-8 locale selection, native Windows execution, unusual native executable paths and process-group/descendant cancellation remain open. Output has no fixed byte quota. These changes do not certify the separate pip-availability helper. |
| Pip availability | The import command inherits stdin/environment/cwd and captures byte streams under a 10 s deadline. Only successful exit reports availability; launch failures, timeout, other statuses and signal exits report false. Captured malformed UTF-8/NUL does not become a text error. | Depends on A44. Real package-manager imports, native Windows, unusual executable paths and descendant/process-group cancellation remain open. |
| Python script execution | Typed lookup results and dedicated subprocess handling. RECORD candidates precede interpreter/sysconfig/user script directories; metadata fallback runs an entry point when no wrapper exists. Executable wrappers run directly, other wrappers run through the selected interpreter. Probe framing requires a line-start marker and a successful status. Child environment removes PYTHONHOME, derives VIRTUAL_ENV from the strict layout and prepends the selected interpreter directory while retaining other variables. | Depends on A36. The pinned payloads and tested native adapters match the recorded corpus. IDA probe model validation, native Windows runtime, malformed object/path conversion, exact outer CLI/OS/timeout diagnostics and process-group interruption remain open. |
| Python interpreter selection | Separate resolution and shared layout helpers. Nonempty HCLI overrides are unchecked; IDAPYTHON overrides require files. Derived candidates prefer version-suffixed names, requested/active virtualenv matches and distinct-prefix environments before validated sys.executable, direct requested-venv and base-prefix fallback. Frozen probes fail with the source message; nullable executable observations retain None semantics. Detailed failure text lists observations and ordered candidates. Successful IDA probes are cached; failed probes can retry. | Depends on A37. The derivation oracle uses actual filesystem predicates over owned fixtures. Malformed probe coercion, all path normalization cases and native Windows execution are not established by these checks. Existence-based derivation can return a directory, matching the source. |
| IDA probe acquisition | Separate orchestration, batch/log parsing and startup-file copying. Existing IDAUSR directories run first with IDA_IS_INTERACTIVE=1; runtime log failures retry in an isolated directory containing cfg/idapython.cfg, ida.reg, idapythonrc.py and top-level license files. Absent or non-directory IDAUSR paths receive one attempt. Results come from the first line-start marker in ida.log, independently of exit status and stdout. Commands inherit stdin/cwd, remove Python/PATH contamination, restore the resolved user virtualenv and wait without a fixed deadline. | Depends on A38. The embedded payload matches upstream; model coercion and caller error classes are not fully equivalent. File copying preserves bytes, permissions and timestamps but does not reproduce every extended attribute or concurrent-error boundary. Real IDA/plugin startup and native Windows execution remain unverified. |
| Python environment creation | Separate orchestration, version/tool planning, target inspection, migration and prompt modules. A successful IDA probe overrides explicit version fallback; malformed explicit versions fail first. Version syntax uses CPython's Unicode decimal digits and final-newline allowance, without the former 3.10 minimum. Registered base interpreters precede PATH fallback; uv receives the registered path when available. PATH fallback tries pythonX.Y, python3 and python, continuing after unsuitable candidates and excluding Store shims. Empty directories are creatable; healthy environments locate python3/python or Windows layout candidates and are reused without migration. Standard-library creation runs ensurepip before final version/pip validation. New environments migrate before configuration; unreadable dependency metadata is skipped, package failures are retained in a successful creation report while OS launch errors abort, and skipped is true only when existing dependencies were declined/skipped. Already-configured interpreter paths avoid configuration writes. JSON sends progress to stderr; interactive migration/configuration support decline and a yes default. Installer-triggered creation probes its supplied installation directly. | Depends on A31. Installed-interpreter execution, exact Rich progress/diagnostics, other-user tilde expansion, terminal signals and native Windows behavior remain open. Concurrent target changes and symlink races are not addressed by these fixtures. Configuration has its own coverage and limits below. |
| Python platform configuration | Separate OS detection, pure per-OS plans, file updates, subprocess execution and command reporting. A typed action selects either a file operation or a program with arguments. Windows uses PowerShell's user Environment API. macOS orders LaunchAgent, launchctl and recognized-shell profile steps. Linux orders systemd environment.d and recognized-shell profile steps with session-specific warnings. Plans are displayed before confirmation. File failures abort at their position; command failures permit later steps but report configured=false. Unchanged files retain their bytes and modification times. Empty/all-skipped successful plans report configured=true with no configured_via. | Depends on A32. Verification receives an explicit child environment; arbitrary later reads of the process environment do not reproduce upstream's os.environ mutation. Literal shell/XML interpolation and fish's broad stale-assignment prefix preserve upstream quirks. Native Windows/Linux persistence, concurrent writers, command signal/exception diagnostics, locale-dependent subprocess decoding and exact Rich rendering remain open. Verification does not certify persistence across login sessions. |
| IDA paths/configuration | Environment search paths, bundle normalization, `ida.default`, canonical IDA config keys, resolution provenance, binary version detection and PE/ELF/Mach-O architecture inspection. Discovery includes the configured installation first, Spotlight/per-user Applications, Linux desktop entries and standard directories, and Windows uninstall metadata/ProgramFiles. Resolved-path deduplication preserves first occurrence. Windows registry metadata supplies names and precedes SDK versions. | Native Windows registry execution, Linux desktop integration, Spotlight failure/timeout fixtures, malformed configuration resolution and complete path expansion remain unverified or incomplete. Discovery uses ASCII installer-version digits; upstream's regex also accepts Unicode decimal digits. Legacy ida64/bundle layouts remain Rust extensions. |
| IDA instance management | Duplicate names preserve existing mappings; manual adds validate and canonicalize directories without implicitly selecting a default. Auto-add preselects new names, requires a nonempty submission and chooses the highest version among newly registered entries. Cancellation preserves configuration bytes. Removal ranks versions numerically with descending-name ties, including stale paths; listing ranks ties by ascending name and reports Valid/Invalid/Missing. Switching preselects the current name and activates only the current platform's idalib library. Deprecated set-default reads/writes idalib configuration independently. Thirteen CLI/PTY fixtures cover these contracts, including malformed-idalib partial switch failure. Protocol setup discovers only when no instances are registered; otherwise it preserves even a missing or stale default. Empty registries receive canonical paths, first-occurrence duplicate names and the highest-version default in one commit. Four additional filesystem tests cover setup and failed persistence. | Exact Rich/questionary rendering differs. Numeric ordering depends on A6. Other-user tilde expansion and exhaustive Click/Clap argument-error precedence remain open. Setup persistence is atomic rather than upstream's incremental writes; setup I/O failures are advisory after successful handler registration. |
| IDB sources | Shared ida source and ke source implementation, ordered display with missing-path markers, canonical paths, duplicate-add success without writes, forced replacement and idempotent removal. Seven isolated CLI tests cover these commands, protocol --force help and actual fixture database launching with ordered or named source selection. Fifteen source-name cases match the pinned upstream ASCII regex, including its single final-newline allowance. | Exact Rich presentation and malformed-value error precedence remain unverified. Hy rejects non-string source values. Native Windows path and filesystem execution remain unverified. Source path existence is checked before command-body name validation, as Click does upstream. |
| IDA installation | Upstream installer options, asset/tag acquisition, license installation, dry-run, edition/service-pack destinations, existing-destination preservation, checked installer subprocesses and required output validation. macOS runs the packaged installer and copies the produced IDA application. Optional instance/default registration only activates idalib for capable editions. Python creation probes the installed directory explicitly. | A local authenticated license round-trip now covers lookup by key, download, publication in the installed product and failed acquisition. Live service access and Windows/Linux installer execution still need verification. EULA uses the upstream idapro/ida_registry API through an external interpreter; compatible-runtime discovery and live idalib execution remain open. Environment creation still needs complete interactive profile-confirmation behavior. |
| Protocol registration | Dedicated command and platform modules; ida protocol register --force and legacy ke setup share registration and empty-registry setup. macOS compiles a quoted login-shell launcher, clears inherited Python environment variables and appends stdout/stderr to ~/Library/Logs/idb_handler.log. A native AppleScript execution test verifies exact URL arguments, environment cleanup and logging without Launch Services registration. Linux uses ~/.local/share/applications/hcli-idb-handler.desktop with mode 0755, environment cleanup, MIME updates and desktop database refreshes; three fixtures verify file contents, quoting, command order and failed MIME registration. Windows adds DefaultIcon and direct native argv registration, and removes known subkeys in upstream order. | Live Launch Services registration/browser clicks, native Windows registry operations and Linux desktop-tool integration remain unverified. Linux fixtures inject desktop-tool execution and do not establish desktop-environment behavior. The old Rust hcli-handler.desktop filename is not automatically migrated. macOS compilation/publication staging remains a Rust extension; a Launch Services failure after publication does not restore the previous application. |
| Ordinary IDA links | URI parsing, filesystem lookup and running-instance matching are separate modules. Ordinary filenames remain percent-encoded literals; original URI spelling is forwarded. Twelve pinned-handler oracle examples cover default resources, empty queries, fragments, authority case, scoped IPv6, invalid textual ports, opaque paths and relative queries without path segments. Relative navigation requires exactly one loaded database; fixtures verify zero/one/multiple cases. Missing or unknown sources do not prevent navigation to an already-open matching database. Existing-instance fixtures accept zero, negative, NaN and infinite launch timeouts without polling analysis. | Unicode authority normalization and exhaustive urllib error precedence remain open. Empty-query/fragment default-resource placement preserves upstream's raw-string append behavior. |
| IDB file lookup | Sources are searched in insertion order. Missing/unreadable directories are skipped; filesystem matching retains extensions and supports filename glob patterns. File symlinks are eligible, including targets outside the source; directory symlinks are not recursively traversed. Legacy idb.search-paths migrate to source-1, source-2, etc. only when sources are empty, and migration commits atomically. Five CLI/IPC fixtures verify literal names, exact extensions, glob lookup/migration, unknown-source running matches, navigation errors and concrete-name startup after glob resolution. The character-class corpus matches 727,831 CPython results. | Depends on A8/A9. Native Windows filesystem behavior, patterns spanning Windows path components, concurrent tree changes and all pathlib-version differences remain unverified. A source is a lookup root, not a confinement boundary; matching file symlinks follow upstream behavior. Migration failure preserves both old in-memory values and destination contents. |
| IDA launch/navigation | Named target and launch options separate URI resolution, installation selection, process launch and waits. The launcher prefers a valid registered default over environment discovery and determines IPC support from SDK/directory versions. Child output is suppressed and Unix launch creates a new session. Startup uses a 0.1 s initial interval, multiplier 1.5 and 2 s cap; the deadline is checked between discovery cycles. Analysis then polls every 5 s without a startup deadline; Ctrl-C skips analysis and continues navigation. Existing matches bypass launch and analysis. Ten CLI/PTY fixtures exercise lookup, selection, detachment, cancellation and navigation. | Live IDA 9.4, native Windows process behavior and macOS open -a runtime remain unverified. PID-specific startup helpers in upstream launcher.py are not called by the active command path and are not treated as required phase deadlines. KE retains exact resolved-path matching after launch; upstream's post-launch wait uses the basename. Version interpretation depends on A6. |
| IDA IPC transport/discovery | Same-user Unix peer validation precedes writes. Unix connection and write timeouts are 2 s; each response read has a fresh 5 s timeout. Fragmented JSON is accumulated with a 1 MiB limit checked before appending. A progressing nine-second response, idle reads, blocked writes and malformed oversized responses have regression coverage. Platform modules enumerate candidates: Unix probes positive PIDs and unlinks stale entries best-effort; Windows enumerates up to 4096 PIDs and probes each named pipe. Candidate order is preserved. Named navigation queries only through the first match; relative navigation queries the full inventory. Owned-socket and isolated-directory tests cover these boundaries. | Depends on A10. Native Windows pipe behavior, PID reuse, permission failures and concurrent socket replacement remain unverified. Unix cleanup follows upstream's unlink policy rather than requiring a socket file type. The 1 MiB limit is a native extension; Windows retains bounded, multi-read transport whereas upstream uses one unbounded 4096-byte read. Split UTF-8 and malformed response coercion are not certified as equivalent. |
| KE downloads | Content-addressed hashes, filename checks, redirect/DNS validation, decoded-byte limits and disk reserve. Payload variants distinguish SHA-256 content from optional JSON metadata. Metadata is attempted first and retains its decoded payload bytes only after character-encoding and syntax validation; HTTP, size, decoding and publication failures warn and permit the IDB transfer. Both payloads require HTTP 200 and publish from a temporary file. Initial host validation precedes cleanup and directory creation; its prepared transport serves both initial metadata and content requests. Thirty-five HTTP/filesystem/dialog/IPC fixtures cover request order, cache preservation, failures, settings, paths, content encoding, cookies and navigation. | Live service, disk exhaustion and publication races remain unverified. Metadata publication is atomic in Hy; upstream writes the sidecar directly and can follow a sidecar symlink. Hy replaces that link and preserves its target. |
| KE HTTP transport | A dedicated module retains one client per validated address and tries candidates in resolver order only after connection failures. Any blocked DNS result rejects the entire set; duplicates are removed without reordering. Client-level retries are disabled. Connect and per-read timeouts are 300 s, allowing a progressing transfer to outlast 300 s. Redirects revalidate each target, release the previous response, allow five hops and accept only the upstream status set. Missing Location reports the HTTP status. Origin checks prevent reusing prepared clients for another host. Direct pinning drops URL userinfo while retaining the host for Host/SNI; the private-host override retains URL authentication. Requests advertise gzip, deflate; reqwest's automatic decoders are explicitly disabled so one KE decoder handles every response. Fourteen focused tests cover transport rules and the IP classifier. | Depends on A8. Native Windows networking, real DNS rebinding, direct TLS/SNI execution and all connection-error mappings remain unverified. HTTPS-to-HTTP redirects are permitted by the pinned source and no longer rejected locally. Environment HTTP(S) proxies use the separate routing path described below. URL/header normalization, explicit default/zero ports, scoped IPv6 and HTTPX write/pool-timeout equivalence remain open. |
| KE proxy routing | A session captures HTTP_PROXY, HTTPS_PROXY, ALL_PROXY and NO_PROXY with HTTPX precedence and CGI handling. An empty environment proxy map falls back to macOS SystemConfiguration or Windows Internet Settings. Environment collection, platform discovery and mount construction are separate modules. Matching uses each rewritten destination URL. Dedicated Hyper connections send absolute HTTP targets or CONNECT to the validated IP; original Host and separate origin/proxy Basic authentication are retained. Explicit rustls layers reproduce the observed outer/inner TLS identities. Per-operation I/O deadlines and owned connection tasks preserve timeout classification and close streams when responses are dropped. Twelve tests cover 186 selection cases, 132 Windows parser comparisons, OS fallback, a macOS snapshot, eight real TLS modes, distinct proxy/origin trust roots, bypass routing, certificate rejection and timeouts. Two CLI tests cover cookies, authentication, bypass and failure preservation. | Depends on A15 and A16. Pooled connection reuse, optional SOCKS, SSL_CERT_DIR, complete URL normalization and certifi/system trust-store equivalence remain open. Proxy DNS is included in the native TCP connection deadline. Native Windows registry and networking behavior is cross-checked only. |
| KE TLS configuration | The session owns separate origin and HTTPS proxy contexts in a dedicated TLS module. SSL_CERT_FILE replacement roots apply to direct requests and CONNECT's inner connection. The outer HTTPS proxy combines file roots with native bundled roots. A twelve-case CPython comparison covers missing, empty, invalid, duplicated and mixed PEM bundles. Generated TLS fixtures test direct custom-root acceptance, untrusted-root rejection and original-host verification; separate proxy/origin certificates verify both layers independently. A CLI test compares trusted/untrusted downloads with HTTPX using the same files and checks old-file preservation. | Depends on A15. SSL_CERT_DIR and exact certifi/OpenSSL system-store equivalence remain open. Rustls and OpenSSL do not have identical certificate-policy, cipher or protocol implementations. Native Windows TLS execution remains unverified. |
| KE cookie sessions | A shared in-memory jar spans metadata, content, redirects and HTTP errors; separate transfers start empty. Tokenization/date parsing, normalization, construction and policy checks remain separate stages. Unicode arbitrary-precision attributes preserve Python's decimal grammar and deferred version conversion. Expiry storage rounds through binary64; normalization failures cancel the response before mutations, while construction overflow preserves earlier expiry deletions. The policy retains domain/path/name insertion order, stable path-length ordering, Secure/Port rules and default paths. Cookie scope follows the rewritten IP URL under pinning. Nine policy tests compare 144 HTTPX cases; three date tests include 7,541 CPython comparisons and two tokenization tests include 7,394 CPython comparisons. Four socket tests and four CLI tests cover session propagation and cached-file preservation. Set-Cookie2 is processed only alongside Set-Cookie; the formats are constructed separately before insertion, with failures isolated to their own batch. | Depends on A8/A12/A14. Exhaustive malformed-header equivalence and runtime-specific CookieJar behavior remain open. Native code does not reproduce Python's warning traceback. Host-only version-zero cookies can reach subdomains; pinned requests to different hostnames can share cookies through the same IP. Negative versions skip the source-domain suffix check, matching the source; their return check also permits the original localhost host in addition to its effective .local name. No public-suffix filter or cookie-storage quota is added. |
| KE content decoding | A separate decoder handles gzip, zlib-wrapped deflate and first-call raw-deflate fallback. Repeated/comma-separated encodings run in reverse order; unknown encodings pass through. Empty network chunks are skipped, but empty output between decoder layers is retained. Size accounting and SHA-256 use decoded bytes; Content-Length no longer causes an encoded-size precheck. Corrupt metadata warns while corrupt content preserves the cached IDB. Five unit tests include 31 HTTPX comparisons; four CLI tests verify decoded bytes, exact limits, corruption and publication. | Depends on A13. Optional Brotli/Zstandard, exhaustive malformed-stream equivalence and transport-dependent chunk boundaries remain open. HTTPX accepts missing trailers and ignores extra gzip members; the native decoder preserves those observed rules. Final-layer size checks precede buffer growth, but intermediate layers and uncapped content can allocate large decoded chunks; there is no total decoder-memory budget. |
| KE metadata | Separate decoding and token adapters feed serde's iterative syntax validator without constructing a value tree. Encoding detection accepts UTF-8/16/32 with Python's BOM/sniffing rules and surrogatepass behavior. Unpaired surrogates are allowed; other malformed byte sequences remain errors. NaN/Infinity/-Infinity are recognized outside strings only at token boundaries. Numeric syntax is validated without floating-point conversion or integer overflow. Six tests include 13,509 CPython comparisons, 4300/4301-digit boundaries and array/object nesting through 900 levels. Nine CLI payloads retain exact original bytes. | Depends on A8/A12. Python's configurable integer limits and stack-dependent recursion failures are not reproduced. Native validation uses an explicit parser stack and accepts a tested 16,384-level document that can exceed Python's recursion budget; upstream can abort the entire transfer on RecursionError whereas native valid syntax continues. Exhaustive codec equivalence and exact diagnostic text remain unverified. |
| KE cache paths | Containment compares resolved path components before confirmation or filesystem writes. Separate Unix and Windows modules resolve missing download tails. Unix follows non-strict CPython realpath behavior, including dangling links, repeated links and cycles; the link cache avoids an arbitrary hop limit. Configured-root aliases and internal directory aliases can receive downloads. Four unit tests provide seven read-only CPython comparisons, including a 64-link chain. | Depends on A8. Windows resolves existing prefixes and rejects unresolved reparse points; dangling reparse points can therefore be rejected where Python permits them. Windows drive, UNC, case and alternate-stream behavior require native execution. Validation is a filesystem snapshot: concurrent link replacement between validation and publication remains unaddressed. |
| KE dialogs | Separate platform command builders and asynchronous process management. Native confirmation precedes DNS and filesystem changes unless explicitly skipped or --no-launch is used. macOS/Windows message text travels in environment variables; Linux confirmation text is markup-escaped. Progress is attempted during transfer. After a successful transfer, the CLI waits 1 s, requests dialog termination and waits up to 2 s for exit before continuing. Transfer and field-validation errors use native error display. Launch/startup failures use a distinct error variant and trigger the native error callback; an IPC navigation rejection retains the upstream terminal-only error path. Unix fixtures exercise decline, tool failure, missing tools, approval, progress dismissal and literal data placement. | Native rendering and window lifecycle on actual OS dialog tools remain unverified. Windows transfer fixtures substitute an invalid PowerShell executable and exercise advisory spawn failure; successful Windows dialogs require native runtime coverage. Linux notification behavior depends on the installed notification service. |
| KE settings/retention | Settings are captured at startup. Boolean values accept Python-trimmed true/yes/on/1 without case sensitivity. Numeric settings reuse the shared arbitrary-precision Unicode parser; malformed values use upstream defaults and nonpositive content limits mean no cap. Retention defaults to 3 days. Integer days are multiplied by 86,400 before float conversion; 113 CPython comparisons check rounding and overflow. A missing cache root returns before conversion. Existing-root conversion overflow aborts before cleanup, while filesystem traversal errors remain advisory. Cleanup removes expired files and then empty directories without recursing through directory symlinks. Three setting tests include 78 upstream comparisons; CLI fixtures exercise Unicode caps, separator whitespace, large signed retention values, conversion overflow and file preservation. | Depends on A11/A12/A14. Native Windows cleanup ordering, concurrent tree changes and exhaustive filesystem failures remain unverified. File symlinks can be unlinked based on target mtime, while targets remain untouched. A negative retention value can expire recently written cache files, matching the pinned source. |
| KE query/navigation | A typed request separates filename, content URL, hash and optional navigation. Repeated decoded url parameters use the final value; a final blank is rejected before filename validation. Remaining query fields retain raw encoding, spelling, flags and fragments. Only ea/rva/name/view request navigation, including blank values and encoded keys. Open-only requests reuse or launch the database and perform the normal startup/analysis wait without sending open_ida_link. Shared control-character normalization avoids divergent ordinary/KE parsing. Twelve pinned query cases and six CLI/IPC scenarios verify the contract. Repeated content-route separators preserve the hash. Filename percent decoding replaces invalid UTF-8, permits colons/C1 characters, and rejects separators, dot names, C0 and DEL; fourteen source-oracle cases and three Unix CLI filenames cover these distinctions. | Depends on A8. Literal query removal deliberately leaves encoded key aliases such as %75rl, following upstream. Windows path/stream semantics require native coverage. Post-launch exact-path matching remains a documented native extension. |
| MCP | Separate orchestration, typed agent/scope, subprocess and listing interpretation modules. IDA plugin upgrade precedes discovery and agent setup. Claude, Codex, Copilot, Pi and OMP preserve upstream command order, scope, installed/marketplace detection and query-failure fallback. Installed Codex/Copilot integrations skip the marketplace query. Process-start failures abort; checked-command failures report the agent status with CLI exit 1. Queries inherit stdin, capture UTF-8 output with replacement and universal newlines, and impose no added timeout. Agent labels, default global scope, command echoes and success output follow upstream text. Forty-five upstream command-sequence comparisons and thirteen CLI scenarios cover these contracts. | Depends on A20. Real agent CLIs, native Windows shim execution, exhaustive PATH/PATHEXT/permission discovery, Rich/questionary terminal behavior, nonstandard/deep JSON and non-UTF-8 paths remain unverified. No output-size or process-duration limit is added. Cancellation after plugin upgrade leaves that upgrade installed, matching upstream. |
| Asset uploads | Shared upload lifecycle for bucket and share commands: 8 KiB streaming SHA-256, preserved optional permission lists, required metadata schema validation, PUT followed by confirmation and canonical leading-slash handling. Ticket URLs use Python truthiness; false-valued URLs skip transfer. Key validation for confirmation follows PUT; required result key/code/version validation follows confirmation. Versions use the shared Pydantic-compatible arbitrary-precision integer model. Failed PUTs never confirm and failed confirmation never reports success. A 75-case source oracle compares status and action order; CLI fixtures verify payloads, hashes, unsigned PUT headers and failure boundaries. | Depends on A22 and A24. Live service round-trip, exhaustive URL/redirect/transport behavior and detailed Pydantic error text remain unverified. Source changes between hashing and PUT are not prevented, matching the upstream two-read lifecycle. |
| Shared files | Typed ACL choices in a dedicated upload module, authenticated prompt default, stdout upload reports, managed-user timestamp updates, lowercased domain permissions and upstream environment-key placeholder behavior. Downloads honor API filenames, explicit destinations and overwrite confirmation. Lookup HTTP, local credential, transport and decoding errors propagate as failures. A source comparison through both API layers verifies that custom APIError classes bypass the lookup's HTTPStatusError catch. Separate table, action and prompt modules support substring search, retained selections, original-order submission, no-match fallback, Ctrl-A toggle-all and Tab inversion. Equal Asset values share selection, including duplicate occurrences after inversion; metadata comparison respects Python numeric equality. Asset and paging integers preserve arbitrary precision and coercion, default only when absent, and reject explicit null. A shared formatter follows upstream's bounded size units. Standalone downloads and deletion print reports to stdout, including missing records/URLs and command error prefixes. Download size reporting happens after publication; deletion reporting precedes confirmation and forced deletion. Conflicting destination flags abort with status 1 before lookup. Explicit output paths use the shared realpath resolver. Overwrite and deletion use distinct Click/Rich input grammars and accept redirected stdin. Arrow/Ctrl-N/Ctrl-P navigation wraps; j/k remain search text. File/action/output cancellation succeeds before actions; output text starts with editable ./ text. Signed pagination passes through. Local HTTP/PTY fixtures cover canonical deletes, ACL tables, batch continuation, duplicate selection, large versions and null rejection. | Depends on A21–A24 and A27. Exact Rich/questionary presentation and output streams, remaining text/confirmation bindings and failure statuses, other-user tilde expansion, complete symlink/path resolution, live accounts and Windows prompts remain unverified. Nonstandard/deep JSON, exhaustive metadata equality, exception-detail text, terminal signals during standalone confirmation, Rich markup in filenames and common download transport remain open. |
| Self-update | Explicit and background checks share paginated release discovery and derive the repository from Rust package metadata or an override. Drafts participate; stable precedence ties retain the last original tag. Invalid requirements fail before HTTP. Metadata message responses end discovery or yield no assets, and an asset count other than one ends without installation. Downloads require HTTP 200, enforce advertised size, stage on the executable filesystem and preserve permissions. Background work has separate worker/cache modules, completion signaling and a 2 s successful-command notification wait. JSON caches use the platform application cache, a strict 24 h timestamp boundary and successful-check-only writes. Six unit tests, six explicit-update CLI tests and one release-profile background fixture cover these contracts; read-only oracles compare thirty release/cache cases. | Depends on A3/A19. Windows publication/rollback and native Windows/Linux cache discovery, concurrent cache writers, published Rust artifacts, HTTPX transport deadlines/redirect equivalence and exhaustive release JSON coercions remain unverified. Rich styling and manual fallback instructions differ. Development builds report Rust rebuild instructions; PyPI mode is unavailable for the native executable. |
| License commands | Exact public-ID, plan and product-code filters; license-key download routes; active-license selection, empty initial checkboxes, catalogue grouping, upstream sorting and per-asset failure continuation. Tables classify decompilers by product subtype and retain complete IDs before terminal-width formatting. Expiration uses the shared Python-compatible datetime parser and separate deterministic formatting module. Local installs support custom paths, creation confirmation, timestamps, permissions and same-file rejection. HTTP/PTY fixtures also cover missing customer IDs and multiple-account selection. | Live licensing, complete Pydantic coercions, exact Rich/questionary rendering, other-user tilde expansion, filesystem extended attributes and Windows hard-link identity remain unverified or incomplete. Datetime coverage depends on A17. Existing customer-ID overrides remain native CLI extensions. Null addon/product/status fields are rendered defensively where upstream table code may raise an exception. |
| Download selection | Case-insensitive patterns, lookaround and numeric/named backreference support, empty-match/invalid-pattern behavior, direct-mode requirements, wrapped/direct tag arrays, exact-before-case-insensitive resolution, suggestions, canonical asset paths and partial-success batches. Tree navigation retains API folder order and permits return to the parent. CLI/PTY fixtures verify request paths and downloaded bytes. | Python regex syntax/Unicode equivalence outside the fixtures remains open; fancy-regex enforces its default 1,000,000 backtrack limit. Exact questionary search and metadata rendering, option-order-dependent Click callbacks, and exhaustive tag/empty-key cases remain unverified. |
| Download cache | A terminal HEAD status below 400 and matching size are required for reuse; existing checksum sidecars are validated. Force bypasses HEAD. Transfers stage in the cache directory and publish only after the complete response; interruption preserves old cache/output bytes and removes staging. Output directories expand the current home. Shared file copying preserves timestamps and permissions. Empty HCLI_CACHE_DIR is ignored; platform paths follow the pinned source. | Cache and sidecar publication are not one transaction; concurrent writers, disk exhaustion, exhaustive redirects/cookies/URL handling, extended attributes and Windows publication need broader verification. Cache keys are confined lexically and filenames require one component, stricter than upstream's direct path joins; existing parent symlinks are not a confinement boundary. New staged cache files use tempfile permissions. |
| API HTTP behavior | JSON GET/POST/DELETE do not follow Location. File GET/HEAD/PUT use an explicit redirect module with a 20-redirect limit, HTTPX method changes, body-header removal on conversion to GET, cross-origin Authorization handling and fragment carryover. Streamed PUT fails when a redirect would require replay; 302/303 continue with bodyless GET. Upload responses are consumed before confirmation, including successful responses. Process-local API cookies are shared across new clients and clones; JSON responses, redirects and file transfers extract cookies before status/body handling. Header decoding uses the full response's UTF-8-or-Latin-1 choice. API clients no longer impose a 60 s overall-response deadline; connect and reqwest read deadlines are 60 s. | Depends on A25–A26. Reqwest's header deadline starts before upload completion, so unlimited-write parity is not established. Partial header reads, connection pooling, exhaustive cookie/pool concurrency, proxy/TLS details, early-response behavior, complete URL normalization, exhaustive Location parsing and exact exceptions remain open. API-key forwarding across origins matches the source fixture; this does not certify every credential scheme. |
| API JSON decoding | GET, POST, DELETE and standalone identity responses use one byte decoder, ignoring HTTP charset declarations as HTTPX Response.json does. UTF-8 BOMs and UTF-16/32 with or without BOMs are recognized; value decoding combines valid surrogate pairs. Integer tokens above 4,300 decimal digits reject the document even in fields ignored by the model. Error bodies use the same parser before selecting messages; invalid JSON uses the upstream status fallback. Encoding detection and number scanning are shared with KE's separate syntax-only validation mode. Body-read and content-decoding failures propagate before HTTP error classification. | Depends on A12/A28. Unpaired-surrogate values cannot be represented by Rust String and remain rejected; NaN/Infinity constants and Python recursion-limit behavior remain unimplemented or unverified. Error prefixes outside the decoded message still differ. |
| API error messages | Present message fields retain string contents or render other values using Python literals, nested repr quoting, dictionary insertion order and binary64 notation. Null becomes None; booleans become True/False; empty strings remain empty. Standard JSON exponents that overflow binary64 display inf/-inf. Fixed 401/403/404/429 messages retain precedence; malformed or missing messages use the status fallback. | Depends on A12/A28/A30. The shared JSON value parser still rejects Python-only NaN/Infinity tokens and unpaired surrogates. Exact Rich rendering, outer exception prefixes and recursive-depth behavior remain open. |
| API content decoding | API and KE use the same gzip/deflate decoder. JSON, signed uploads and intermediate redirects decode before interpretation; final downloads stream decoded bytes into the staged cache writer and checksum. Both API client construction paths advertise gzip, deflate and explicitly disable reqwest automatic decoding. Original Content-Encoding and Content-Length headers remain available. Streamed HTTP errors use the upstream status fallback without consuming their bodies. | Depends on A13/A29. Optional Brotli/Zstandard remain unsupported. Encoded HEAD length can differ from decoded cache size and cause repeated downloads, matching upstream. Intermediate layer expansion and buffered JSON/redirect/PUT responses have no independent memory quota. Native staged publication deliberately preserves old files on decoding failure; upstream writes the cache directly. |
| Extensions | A selected Python runtime discovers hcli.extensions and registers them once against the actual upstream Click host. The same process handles new/replaced commands, changed execution attributes and removed paths; it preserves terminal streams, literal arguments, root authentication context and exit status. Native commands receive extension help/inventory metadata and remain in Rust when their execution path is unchanged. Creation prints the template command and rejects a project-name argument. Seven opt-in integration tests pass against the pinned Python host; two interpreter-configuration tests run in the default suite. See extensions.md. | Depends on A7. Windows runtime execution, custom lazy groups/parsers, in-place mutation of callback internals or shared globals, and custom root-help rewrites need more coverage. The bridge observes command/parameter attributes rather than proving equivalence of arbitrary Python mutation. Changed parent execution attributes delegate their subtree to Python. Startup includes interpreter/entry-point work; no registration deadline is imposed. No separate native extension ABI is implemented. |
| Root CLI and help | Separate parsing and local status modules. Native status reports configured auth identity, default/missing/unselected IDA installations and independent idalib state without authentication requests or persistence. Without installed Python extensions, malformed configuration does not prevent help. Version output honors version/suffix/binary-name overrides; command inventory is sorted, excludes hidden/extension-removed leaves and reports its count. Extension names/versions and added/nested command names appear in help/inventory. The ineffective native global quiet option was removed; upstream has no such flag. | Exact Rich/Clap rendering, nested version-option differences and exhaustive error precedence remain open. Installed extension imports/registration have their upstream startup side effects, including on help requests. Status does not certify live authentication. Windows registry precedence is implemented but requires native verification. |

## Primary provenance

All comparisons refer to local files at the pinned upstream revision:

- `src/hcli/main.py::_get_status_section`, `get_help_text` and root options; `src/hcli/commands/commands.py`: local help status, environment-based identity/version, sorted visible command leaves and absence of a global quiet flag.
- `src/hcli/lib/ida/__init__.py::parse_version_from_ida_pro_py`, `parse_instance_version`, `select_default_ida_instance`, `find_standard_installations` and platform discovery helpers: Windows installer metadata precedes SDK/binary/path/name fallbacks; version/default ordering and installation discovery preserve the documented source order. Help reads local metadata without executing IDA or importing its Python code.
- `src/hcli/commands/ida/{add,remove,list,switch,set_default}.py`: duplicate handling, manual versus automatic default selection, cancellation, distinct list/default tie-breakers, platform idalib activation and deprecated configuration behavior.
- `src/hcli/lib/ida/python/platform_env.py` and `src/hcli/commands/ida/python/create_environment.py::configure_env_var`: platform plans, shell/session detection, text-file updates, execution ordering, verification and configuration reports. Source file operations run only against the in-memory adapter in `src/ida/python/platform/tests/reference.py`.
- `src/hcli/lib/ida/python/environment.py::{collect_python_environment_state,check_python_environment,identify_setup_pattern}`, `src/hcli/lib/ida/python/__init__.py` path/probe helpers, `src/hcli/lib/venv.py` configuration/cache helpers and `src/hcli/commands/ida/python/doctor.py`: doctor observations, complete finding text, classification precedence, context notes and rendering. Source policy and rendering functions are loaded from AST without executing their command entry points.
- `src/hcli/lib/ida/python/environment.py::{validate_python_environment,warn_python_environment,format_environment_warnings,format_environment_findings_plain}`, `src/hcli/lib/ida/plugin/install.py::{resolve_python_for_dependencies,validate_can_install_python_dependencies,install_single_plugin_dependencies}`, its exceptions module and `src/hcli/commands/ida/python/{__init__,_common}.py`: guard ordering, skip behavior, advisory execution, mandatory pip checks and migration's direct installation path.
- `src/hcli/lib/ida/python/__init__.py::{PipOptions,merge_bundle_pip_options,verify_pip_can_install_packages,pip_install_packages,_format_pip_error,_raise_for_known_pip_errors}` and `src/hcli/lib/ida/plugin/bundle.py::bundle_dependency_source`: pip argv, default/bundle option precedence, inherited subprocess context and ordered error classification. The oracle records subprocess calls in memory and never runs pip.
- `src/hcli/commands/plugin/__init__.py::plugin` and CPython 3.13.15 `pathlib/_local.py::{PurePath._parse_path,PurePath._format_parsed_parts,Path.expanduser}`, `posixpath.expanduser`, `ntpath::{splitroot,join,split,expanduser}`: local find-links conversion. The source expression and methods execute with in-memory environment adapters; account lookup is read-only.
- `src/hcli/commands/plugin/bundle.py::_download_wheelhouse`, `src/hcli/lib/ida/plugin/bundle.py::PipTarget` and packaging 26.0 `tags.py` from upstream `uv.lock`: separate bundle download argv, wheel tags and raw byte diagnostics. Subprocess calls are recorded in memory; the oracle rejects a packaging version different from the lockfile.
- `src/hcli/commands/plugin/bundle.py::_resolve_targets`, `src/hcli/lib/ida/plugin/bundle.py::{resolve_platform_alias,_parse_python_version,PipTarget}`, `src/hcli/lib/ida/python/__init__.py::detect_current_python_version` and `src/hcli/lib/venv.py::probe_python_version`: target input grammar, validation order, alias preservation, deduplication and version-only interpreter detection. Selector imports are replaced by in-memory observation callbacks without replacing its control flow.

- `src/hcli/lib/ida/python/__init__.py::{GET_SCRIPT_INFO_PY,RUN_ENTRY_POINT_PY,ScriptInfo,EntryPoint,_run_probe,render_script_not_found,get_environment_for_python,run_in_python_environment,run_script}` and `src/hcli/commands/ida/python/{find_script,run_script}.py`: script lookup order, result framing, wrapper/entry-point dispatch, environment reconstruction and child status. Embedded interpreter payloads are compared directly with these pinned constants; fixtures do not modify the source checkout.
- `src/hcli/lib/ida/python/__init__.py::{_derive_python_exe,resolve_current_python,probe_current_python_info,GET_PYTHON_INFO_PY}`, `src/hcli/lib/venv.py::get_python_exe_candidates` and `src/hcli/env.py::_env_optional`: interpreter selection, cache lifecycle, frozen/nullable observations and empty override semantics. The derivation oracle loads selected AST definitions and reads Rust-owned filesystem fixtures.
- `src/hcli/lib/ida/__init__.py::{run_py_in_current_idapython,_run_ida_batch_script,_clean_env_for_idat,_prepare_headless_ida_user_dir}` and `src/hcli/lib/ida/python/__init__.py::GET_PYTHON_INFO_PY`: startup context, retry boundary, environment cleanup, log framing and isolated file selection. Source writes are intercepted by the read-only oracle adapter.
- `src/hcli/lib/ida/python/environment.py` report collectors/models/notes, `src/hcli/lib/ida/python/__init__.py::{find_python_version_mismatches,format_python_version_mismatch_warning}`, `src/hcli/lib/venv.py` version helpers and `src/hcli/commands/ida/python/explain_environment.py`: explain observation order, mismatch policy and full rendering. Source tests use actual report models with the explicit native-runtime identity assumption.
- `src/hcli/commands/download.py`, `src/hcli/lib/api/asset.py::get_tags`, `src/hcli/lib/api/common.py::download_file` and `src/hcli/lib/util/cache.py`: case-insensitive matching, tag response alternatives, folder order, selected-key normalization, HEAD validation, cache roots and metadata-preserving copies.
- `fancy-regex` 0.16.2 package source in the local Cargo registry: lookaround/backreference APIs and default backtrack limit. The locked dependency adds `bit-set` 0.8.0 and `bit-vec` 0.8.0; unrelated platform dependency versions are retained.
- `src/hcli/lib/extensions/__init__.py`, `src/hcli/main.py` and `src/hcli/commands/extension/{create,list}.py`: Python entry-point discovery, version metadata, registration against the actual Click root, root authentication context and template output. The compatibility tests install this checkout into an isolated interpreter and add temporary entry-point metadata.

- `src/hcli/lib/api/license.py`, `src/hcli/lib/api/customer.py`, `src/hcli/commands/license/{common,get,list,install}.py`, `src/hcli/commands/common.py` and `src/hcli/lib/ida/__init__.py::install_license`: exact filtering, public-ID versus key semantics, asset failure handling, customer selection, grouping, display metadata and `shutil.copy2` installation. The actual license sort puts null dates first because it reverses `(end_date is None, end_date)`, despite its contrary comment.

- `src/hcli/lib/api/asset.py`, `src/hcli/lib/api/common.py`, `src/hcli/commands/asset/{put,delete}.py`, `src/hcli/commands/share/{put,get,list,delete}.py` and `src/hcli/lib/util/string.py`: upload payloads, checksums, required response and bucket fields, leading-slash key normalization, ACL mapping, domain normalization, optional lookup semantics, output filenames and confirmation behavior.

- `src/hcli/lib/ida/plugin/__init__.py`: manifest, settings and model validation.
- `tests/fixtures/manifest-validation.json`: 16 accepted and 42 rejected cases evaluated against the pinned upstream model definitions, with `pydantic` 2.12.5 and `semantic-version` 2.10.0 from upstream's lockfile. The oracle check requires `ValidationError` for rejected cases, rejects unexpected exceptions, and verifies specified normalized values. The Rust regression consumes the same inputs and outcomes.
- `tests/fixtures/plugin-files.json`: 38 cases and 114 checks evaluated against the pinned archive and directory validators. The check loads those functions from the local source AST, creates ZIPs in memory and substitutes a read-only directory lookup; it does not execute plugin code or write files. The Rust regression tests the same outcomes, including all six platforms and nested roots.
- `src/hcli/commands/plugin/lint.py`: directory/archive inspection, README naming, extra metadata and contact recommendations.
- `src/hcli/commands/update.py` and `src/hcli/lib/update/release.py`: update modes, original tags, development indicators, pagination, asset filtering, byte-count validation and replacement rollback. The Rust asset-name mask retains the upstream OS/architecture convention; fixture assets exercise that convention without claiming published artifact compatibility.
- `src/hcli/lib/update/version.py` and `src/hcli/main.py::handle_command_completion`: background release selection, application cache location/schema, strict timestamp comparison, silent errors, cached-result comparison and successful-command 2 s notification wait. Read-only probes use the installed HCLI 0.24.0 functions; the cache oracle substitutes only the clock and in-memory file reads.
- `src/hcli/lib/config/__init__.py`: flat configuration keys, migration, write errors and malformed-file fallback. Hy intentionally rejects malformed files instead of adopting empty defaults.
- `src/hcli/lib/constants/auth.py`: string timestamps, insertion-ordered credential mappings, default reassignment, name selection and model defaults. `create_credentials` appends `Z` to an already timezone-qualified ISO timestamp; Hy preserves existing timestamp strings and generates RFC 3339 strings for new timestamps.
- `src/hcli/lib/auth/__init__.py`: the current lightweight GoTrue client, `/user` validation, token/OTP/logout request headers, minimal token responses, credential selection and mutation. This revision no longer uses the Supabase SDK; legacy session-key compatibility must not be described as current upstream session persistence. Its GoTrue client and AuthService hold distinct session fields, which affects stored-token sign-out.
- `src/hcli/lib/commands/__init__.py`, `AuthCommand.invoke` and `require_auth`: constrained-command option checks versus ordinary authentication checks. Decorator use in `commands/download.py`, `commands/share/`, `commands/license/{list,get}.py`, `commands/asset/` and `commands/auth/key/list.py` defines the constrained command set.
- `src/hcli/commands/login.py`, `logout.py` and `whoami.py`: terminal selection, forced login, credential removal confirmations, single-credential automatic removal and status presentation.
- `src/hcli/lib/auth/__init__.py::{get_user,show_login_info}` resolves environment-key email only when no asyncio loop is running; its exception fallback is api-key-user. `commands/whoami.py` and `commands/auth/default.py` are synchronous, whereas login, auth switch and key installation use async_command. `lib/api/auth.py::AuthAPI.whoami` parses the required string email field. `lib/api/common.py::{APIClient.__init__,_handle_response,get_json}` supplies JSON headers, 60 s inactivity limits, no automatic redirect following and error classification. A 302 response with valid identity JSON can therefore supply an email without following Location.
- Repository-wide call-site inspection finds managed AuthService.get_user use in share/put; show_login_info calls it only for an environment key with no managed source. Other get_user occurrences belong to GoTrue token validation. Consequently managed whoami does not touch last_used. CLI fixtures verify unchanged configuration bytes for both managed keys and interactive credentials; share-upload timestamp coverage remains in asset_upload.rs. The prior audit's generic last-used gap outside share uploads was unsupported and has been removed.
- `src/hcli/lib/auth/__init__.py`, `_start_oauth_server` and `HTML_PAGE`: callback routing, token delivery and the 120-second login deadline. The Rust implementation uses [Hyper 1.10.1 HTTP/1 server support](https://docs.rs/hyper/1.10.1/hyper/server/conn/http1/struct.Builder.html) and [http-body-util 0.1.3 Limited](https://docs.rs/http-body-util/0.1.3/http_body_util/struct.Limited.html), both already present in the lockfile through the HTTP client. Enabling server support adds `httpdate` 1.0.3.
- `src/hcli/commands/auth/key/install.py`: canonical `--key-name`, validation, default selection and failure exit behavior.
- `uv.lock`: pins `semantic-version` 2.10.0, used to produce `tests/fixtures/semantic-version.json` by evaluating `Version.coerce` and `SimpleSpec` membership. The fixture records normalized values, specification validity and matching input indices; the Rust test checks all of these independently of its implementation.
- `uv.lock`: pins `packaging` 26.0. `tests/fixtures/macos-platform-tags.json` records `packaging.tags.mac_platforms((10, 13), "x86_64")` and `mac_platforms((11, 0), "arm64")`, the targets selected by upstream's bundle implementation.
- `src/hcli/commands/plugin/__init__.py`: repository and pip group options.
- `src/hcli/commands/plugin/install.py` and `upgrade.py`: upgrade identity, no-op and configuration-failure behavior.
- `src/hcli/commands/plugin/config.py` and `_prompt.py`: configuration CLI, defaults and interactive prompts.
- `src/hcli/lib/ida/plugin/settings.py`: parsing before validation, canonical configuration identity and no-op updates. `src/util/python_regex.rs` records 25 cases checked against CPython 3.13 `re.match`; these establish only the represented expression semantics.
- `inquire` 0.9.4 uses an editable [initial text value](https://docs.rs/inquire/0.9.4/inquire/struct.Text.html) and a [hidden password prompt](https://docs.rs/inquire/0.9.4/inquire/struct.Password.html). Hy disables password confirmation and handles cancellation as a returned error. Local `console` 0.15.11 Unix source raises SIGINT during interactive key reads; replacing the setting prompts allows the existing installation cleanup path to execute. Other prompt families retain their existing implementations.
- `src/hcli/lib/ida/plugin/settings.py`: setting lookup, validation and deletion.
- `src/hcli/lib/ida/plugin/install.py`: dependency preflight and filesystem publication; `pack_plugin_directory_to_zip` defines copying/filter semantics; `install_plugin_directory_editable`, `_get_ida_site_packages_dir`, `_write_editable_pth_file` and `_remove_editable_pth_file` define package registration and cleanup. `get_installed_plugin_records`, `is_valid_plugin_directory`, `get_installed_minimal_plugins` and `get_installed_legacy_plugins` define the distinct installation inventories. `uninstall_plugin` and `_uninstall_broken_plugin_directory` define source preservation and remnant removal.
- `src/hcli/commands/plugin/status.py`: named versus unfiltered inventory, request ordering, JSON entry kinds and missing-name exit status.
- `src/hcli/lib/ida/plugin/repo/aggregate.py`: repository identity filtering.
- `src/hcli/lib/ida/plugin/reference.py`: reference grammar and identity normalization.
- `src/hcli/lib/ida/plugin/repo/github.py`: GitHub code search, release/tag queries, archive filters and caches.
- `src/hcli/lib/ida/plugin/repo/fs.py`: recursive archive discovery.
- `src/hcli/commands/plugin/bundle.py`, `src/hcli/lib/ida/plugin/bundle.py`, and `src/hcli/lib/ida/plugin/repo/bundle.py`: bundle source resolution, target tags, pip arguments, manifests and wheelhouse extraction.
- `src/hcli/lib/ida/python/__init__.py`, `merge_bundle_pip_options`: bundle source precedence, isolated pip execution and explicit offline selection.
- `src/hcli/lib/ida/__init__.py`: IDA configuration keys and repository defaults.
- `src/hcli/lib/ida/__init__.py`, `find_current_ida_platform`: executable-based platform selection and environment override precedence.
- `src/hcli/lib/ida/python/environment.py`: diagnostic models and findings.
- `src/hcli/commands/ida/python/`: interpreter execution and environment commands.
- `src/hcli/commands/ida/install.py`: installer CLI and lifecycle.
- `src/hcli/lib/ida/handler/ke_url_handler.py::{handle,_has_nav_params,_strip_query_param,_sha_from_content_url}` and `src/hcli/lib/ida/resolve.py::resolve_and_navigate`: last-value query selection, raw literal-key removal, open-only operation and launch-error callback boundaries. Twelve query cases and a repeated-route-separator case were evaluated by executing only the pinned helper definitions extracted from the source AST under CPython 3.13.15. That oracle read files but did not write any files.
- `src/hcli/lib/ida/handler/ke_url_handler.py::{_confirm_open_dialog,_show_download_dialog,_dismiss_dialog,_show_error_dialog,_cleanup_old_downloads}` and `src/hcli/env.py`: native dialog command arguments, literal environment values, confirmation/validation/cleanup ordering, 1 s successful-download delay, 2 s dismissal wait, retention defaults and environment parsing.
- `src/hcli/env.py::{_env_int,_env_bool}` strips Python whitespace before conversion. In particular U+001C..U+001F are removed by str.strip even though int alone rejects them. The native settings reader shares these whitespace rules with HTTP header parsing. `_cleanup_old_downloads` multiplies integer days by 24 × 60 × 60 before subtracting from time.time(); conversion sits outside its filesystem exception handler. A read-only oracle compares exact binary64 cutoff bits or conversion failure for 113 values, and the installed upstream environment reader supplies 78 setting comparisons.
- `ke_url_handler.py::{_validate_content_url,_is_blocked_ip,_pinned_request_args,_connect_first_pinned,_open_validated_stream}` and CPython 3.13.15 `ipaddress.py`: all-address rejection, ordered deduplication, mapped/6to4 recursion, Teredo/CGNAT additions, registry exceptions, credential stripping and redirect revalidation. The handler constructs httpx.Client(timeout=300.0). Installed reqwest 0.12.28 `async_impl/client.rs::{read_timeout,connect_timeout,retry}`, `dns/resolve.rs` and `error.rs::is_connect` establish the native configuration and classification mechanisms; local socket tests verify their observed behavior. Installed HTTPX `_client.py` enables trust_env; native routing now reads environment HTTP(S) proxies with macOS/Windows fallback when the environment proxy map is empty.
- HTTPX 0.28.1 `_models.py::Cookies` wraps request URLs in urllib requests. CPython 3.13.15 `http/cookiejar.py::{parse_ns_headers,CookieJar._normalized_cookie_tuples,CookieJar._cookie_from_cookie_tuple,CookieJar._process_rfc2109_cookies,CookieJar.extract_cookies,CookieJar._cookie_attrs,DefaultCookiePolicy}` defines the cookie policy. The upstream IP-authority rewrite determines cookie scope independently of Host/SNI. Expiry parsing follows `http2time`, `_str2time`, `_timegm` and `offset_from_tz_string`; `calendar.timegm` adds day/hour/second components to the month's first day. Native conversion uses existing regex/chrono dependencies; the generic cookie dependency has been removed.
- `Cookie.__init__` stores expires via int(float(expires)); `_cookie_from_cookie_tuple` deletes expired entries before calling that constructor. `_normalized_cookie_tuples` catches ValueError from Max-Age text but lets TypeError from a missing value cancel normalization. `set_ok_domain` conditions the source-domain suffix check on version zero, and `set_ok_port` validates integers before comparing their original spelling. Native integers use num-bigint 0.4.8 and num-traits 0.2.19; binary64 conversion is verified around 2^53 and the finite/infinite rounding boundary. The decimal table is derived from the isolated runtime's unicodedata.decimal and checked across all code points.
- CookieJar.make_cookies parses Set-Cookie2 before Netscape headers when both header types occur, despite the default policy disabling RFC 2965. split_header_words handles quoted/escaped values and comma-separated cookies. Default policy accepts legacy version-zero and negative-version cookies, while rejecting missing/positive versions after construction. Set-Cookie2 Expires remains a string and can fail construction; Max-Age can replace it first. The original combined-header probe now returns the same b=two; a=one header in Hy, with 29 additional mixed-format oracle cases covering selection, ordering, defaults and failure isolation.
- HTTPX `_utils.py::{get_environment_proxies,URLPattern}` and CPython urllib.request.getproxies_environment define proxy selection. Lowercase-suffix variables override other casing; CGI REQUEST_METHOD suppresses the uppercase HTTP proxy; NO_PROXY patterns have port/host/scheme priority and an asterisk disables all mounts. Matching sees the URL after IP rewriting. A read-only 186-case oracle compares selected proxy origins or configuration failure. HTTPX normalizes default ports before lowercasing schemes, so uppercase bypass patterns can retain explicit default ports.
- CPython 3.13.15 [`Modules/_scproxy.c`](https://raw.githubusercontent.com/python/cpython/v3.13.15/Modules/_scproxy.c), `set_proxy` and `get_proxies`, supplies the macOS snapshot mapping. Installed `urllib/request.py::getproxies_registry` supplies Windows parsing and partial-result behavior. HTTPX consumes getproxies(), without invoking urllib's separate OS bypass routines. Native discovery consequently does not evaluate PAC or translate macOS exception lists or Windows ProxyOverride into bypass mounts.
- HTTPX 0.28.1 `_models.py::{Response._get_content_decoder,Response.iter_bytes}` and `_decoders.py::{GZipDecoder,DeflateDecoder,MultiDecoder,ByteChunker}` define encoding selection, reverse composition, first-call fallback, empty-chunk handling and trailer behavior. The pinned HCLI handler consumes iter_bytes before applying byte limits, metadata validation and SHA-256. Native decoding uses flate2 1.1.9 with the pure-Rust zlib-rs 0.6.7 backend; Cargo.lock retains both versions. A read-only HTTPX oracle consumes the same 31 encoded/chunked cases and compares decoded bytes or decoding failure.
- The IP digest evaluates 86,016 IPv4 addresses, followed by 196,608 IPv6 addresses, in `transport/tests.rs::ip_classification_matches_the_pinned_python_digest` order. IPv4 varies the first octet over 0..255, second over [0,16,18,19,31,51,63,64,100,127,128,168,254,255], third over [0,2,51,100,113,255], and last over [0,9,10,255]. For each 16-bit p, IPv6 tests [p,0,0,0,0,0,0,1], [2001,p,0,0,0,0,0,1] and [3fff,p,0,0,0,0,0,1], with fixed hextets written in hexadecimal. Executing only the pinned upstream helper extracted from its AST produced 73,896 blocked results and SHA-256 `12cf5623b3f909af1e07f24a441f7889f49f146373b2eff28cc487623323fc5d` over the 282,624 result bytes. Separate examples cover narrow IPv6 exceptions and embedded IPv4 addresses.
- `src/hcli/lib/ida/handler/ke_url_handler.py::{_download_metadata,_download_file,_idb_name_from_path}`: optional metadata before required content, 16 MiB metadata limit, JSON validation, exact HTTP 200, content hashing and independent publication. CPython 3.13.15 `json/__init__.py::{detect_encoding,loads}`, `json/decoder.py` and `json/scanner.py` define byte detection, surrogatepass, constants and number grammar; sys.get_int_max_str_digits() returned 4300 in the verified runtime. Native decoding validates bytes before serde_json 1.0.150 `de.rs::ignore_value` checks syntax iteratively. Its `read.rs::ignore_str` does not validate UTF-8, so it is never used directly on unvalidated download bytes.
- The metadata token corpus enumerates bodies of length 0 through 3 over the ordered fragments `0`, `-`, `1`, `.`, `e`, `NaN`, `Infinity`, `"`, `\`, space, `,`, `:`, `[`, `]`, `{`, `}`. Each body is tested as a root value, array member and object value, in that order. The 3 × (1 + 16 + 16² + 16³) = 13,107 acceptance bytes contain 266 successes and have SHA-256 `f961a037e6714b38158435ece87c6f6b085b5c0a1870bd0e6bc2725f7e3f0720`. An additional 402 cases cover encodings, strings, token boundaries, integer limits and depth. Both the fixed digest and all case outcomes were compared with read-only CPython processes.
- `ke_url_handler.py::{handle,_idb_name_from_path}` and CPython 3.13.15 `posixpath.py::realpath`: resolved containment, replacement decoding and the C0/DEL filename exclusion. Fourteen filename cases were evaluated from the pinned helper; seven path comparisons run the installed interpreter read-only against Rust-created fixtures. Windows uses [MetadataExt::file_attributes](https://doc.rust-lang.org/std/os/windows/fs/trait.MetadataExt.html#tymethod.file_attributes) and Microsoft's [FILE_ATTRIBUTE_REPARSE_POINT definition](https://learn.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants) to reject unresolved reparse points before falling back to an existing prefix.
- `src/hcli/lib/ida/ipc.py`: response fields, JSON truthiness, connected-peer validation, 2 s Unix connection/write timeout and 5 s per-read timeout.
- `src/hcli/lib/ida/ipc.py::{_discover_unix,_discover_windows,_is_process_alive}`: process liveness, best-effort stale-entry unlink, enumeration order and the fixed 4096-PID Windows probe. [Microsoft EnumProcesses documentation](https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-enumprocesses) confirms that its array capacity and returned count are expressed in bytes; dividing returned bytes by sizeof(DWORD) yields the PID count. No retry beyond the upstream capacity is added.
- `src/hcli/lib/ida/launcher.py::{find_ida_binary,get_ida_version,launch_and_wait,_wait_for_idb_instance,_wait_for_analysis_on_instance}` and `resolve.py::resolve_and_navigate`: selected-installation precedence, detached launch, exponential startup polling, separate analysis wait, Ctrl-C skip and existing-instance bypass.
- `src/hcli/lib/ida/protocol.py`: operating-system protocol registration.
- `src/hcli/commands/ida/protocol/register.py`: existing-instance short circuit, unconditional handler reinstall despite --force, discovery, default selection and advisory setup I/O errors.
- `src/hcli/commands/ida/source/{add,list,remove}.py`: source insertion order, canonical paths, duplicate/missing-name no-op statuses and name validation. The 15 source-name regression cases were checked by importing the pinned upstream NAME_PATTERN and RESERVED_NAMES in the isolated Python runtime.
- `src/hcli/lib/ida/handler/default_url_handler.py`, `launcher.py` and `resolve.py`: raw URI parsing/default-resource behavior, relative-instance cardinality, pathlib filename lookup, legacy search-path migration and distinct filesystem/running-name matching. Twelve handler examples were recorded with only the final relative/named dispatch methods replaced by recording callbacks.
- CPython 3.13.15 `fnmatch.py` and `glob.py` from the installed interpreter: character classes, reversed-range removal, subsequent negation, symlink policy and recursive traversal. The native corpus enumerates class bodies of length 0 through 6 over `a-z!]^`, then tests 13 candidate characters in a fixed order: 13 × (1 + 6 + … + 6⁶) = 727,831 results. SHA-256 of the corresponding 0/1 result bytes is `36f3dce5242eabe75f3de48956f7522b42d9bc20232591649f14c3e823a8b92a`; the native test reproduces that digest without requiring Python.
- `src/hcli/commands/mcp/install.py`: discovery order/display names, scope defaults, exact subprocess arguments and capture rules, installed/marketplace detection, failure boundaries and upgrade-before-selection ordering. Optional oracles invoke the actual installed setup functions with subprocess execution replaced by an in-memory recorder; no real agent configuration is modified. CPython casefold enumeration identifies eleven non-ASCII-to-ASCII mappings.
- `docs/schemas/ida-plugin.json`: vendored schema.

Metadata uses Serde's associated-function derive to decode fields once, followed
by cross-field validation in the `Deserialize` implementation. This avoids a
second copy of the model's fields. The mechanism is described in
[Serde's remote-derive documentation](https://serde.rs/remote-derive.html).

Reproduce the change inventory with:

```sh
git -C /Users/int/dev/ida-hcli diff --stat v0.18.1..HEAD -- src/hcli
git -C /Users/int/dev/ida-hcli log --oneline v0.18.1..HEAD -- src/hcli
```

Proxy TLS provenance: HTTPcore 1.0.9 `_sync/http_proxy.py::TunnelHTTPConnection`
uses the rewritten remote-origin host for TLS after CONNECT. Its outer HTTPConnection
receives the request's sni_hostname extension, including when that connection targets
an HTTPS proxy. `_sync/connection.py::_connect` applies that extension to start_tls.
A read-only fake network backend records these arguments for all eight combinations
of pinning, HTTP/HTTPS proxy and HTTP/HTTPS destination. The native test uses generated
certificates and real loopback TLS to verify SNI and request targets; an additional
DNS-only certificate is rejected for a pinned IP tunnel. The earlier review's assumption
that CONNECT preserved original-host TLS identity is superseded by this evidence.

HTTPX `_config.py::create_ssl_context` and HTTPcore `_ssl.py::default_ssl_context`
also establish a remaining trust-store difference: origin and proxy contexts can
combine certifi, environment and system roots differently. The session now owns
separate native contexts. A nonempty SSL_CERT_FILE replaces origin roots for both
direct requests and the inner CONNECT connection. The HTTPS proxy's outer context
combines that file with webpki roots. Without a file, both use webpki roots.
HTTPX's [environment-variable documentation](https://www.python-httpx.org/environment_variables/)
also specifies the origin-file override and the OpenSSL directory layout required
for SSL_CERT_DIR. Directory and OpenSSL system-root loading remain unimplemented;
exact trust-store and certificate-policy equivalence has not been established.
The installed reqwest 0.12.28 `ClientBuilder::use_preconfigured_tls` accepts the native
rustls ClientConfig, so direct and tunneled origin requests share the same configured
trust source. Real TLS sockets verify that direct IP pinning retains the original
DNS name for SNI and certificate checks.

Datetime provenance: CPython 3.13.15 `datetime.fromisoformat`, its installed
`_pydatetime.py` calendar/separator routines, and the
[Python datetime documentation](https://docs.python.org/3.13/library/datetime.html#datetime.datetime.fromisoformat)
define the comparison target. The native corpus uses the accelerated runtime as
the oracle, including behavior that differs from the pure-Python fallback.
Upstream `commands/license/common.py::license_to_string`, `license/list.py`,
`auth/list.py`, `auth/key/list.py::{format_datetime,format_relative_time}` and
`share/list.py::format_date_time` supply the distinct report policies. Tests execute
the relevant installed formatter functions with a fixed datetime class; source
files and system clocks are not modified. License end dates remain optional strings
in `lib/api/license.py`, and sorting continues to use their original lexical values.

## Bounded additional findings

- **Medium:** The previous shared-download text helper returned ./ after prompt
  errors. The replacement propagates cancellation before creating an output
  directory or requesting download information. Ctrl-C fixtures cover file,
  action and directory prompts; the file picker also handles Ctrl-Q.
- **Medium:** Search only changes visible choices. Selected files remain selected
  when hidden, Ctrl-A and Tab affect every file, and submission follows API order.
  A filter with no matches shows all files again, matching questionary. Terminal
  fixtures verify the exact DELETE keys sent after these transitions.
- **Medium:** Restoring canonical terminal input between individual key reads
  allowed queued characters to be edited by the terminal and Ctrl-C to arrive as
  a signal. The picker now owns raw mode across its complete lifetime and restores
  the prior mode on return or error. Crossterm preserves Ctrl-A separately from
  Home; the existing dependency is now declared directly for this input handling.

- **Medium:** Treating an executable-start error as an empty MCP listing could
  trigger an installation attempt after failed discovery. Query spawn errors now
  abort immediately; a completed query with a nonzero status remains absent data,
  as upstream specifies. Checked-command failures stop later setup commands and
  report exit 1 at the HCLI boundary, retaining the child's status in the message.
- **Medium:** MCP installs/upgrades the IDA plugin before asking for an agent.
  Cancelling selection or failing agent setup therefore leaves the completed
  plugin upgrade installed. Thirteen CLI scenarios verify this ordering, including
  a failed plugin download that prevents any agent execution.
- **Low:** Agent queries previously imposed a 30 s timeout and closed stdin.
  They now inherit stdin and have no extra deadline, matching subprocess.run.
  A real agent can consequently wait for input or produce unbounded captured output.

- **Medium:** The prior background checker queried `/releases/latest`, skipping
  paginated selection and development-tag policy. It also recorded failed checks
  as completed, suppressing retries for 24 h. Background checks now share release
  selection and save cache data only after a successful check.
- **Medium:** Upstream treats release metadata containing a `message` field as no
  available release/assets, even for HTTP errors. Missing or ambiguous platform
  assets likewise produce a successful command without installation. These exits
  do not certify that the installed executable is the newest published artifact.
- **Medium:** Draft releases and the last equal-precedence tag now participate in
  selection. Authenticated repository responses may therefore select a draft that
  an unauthenticated response would omit. Downloads still require HTTP 200 and
  exact advertised size; 201/206 bodies cannot replace the executable.
- **Low:** Background cache files now use platformdirs-style application paths and
  `update_check.json`, independently of HCLI_CACHE_DIR. The previous native
  `updates/last_check` marker no longer suppresses discovery. Under A19, a recent
  JSON cache suppresses another check without replaying an update notification.

- **Medium:** Lint previously resolved dependency files, allowing missing requirement
  files or malformed inline TOML to fail inspection even though upstream lint never
  reads those dependencies. Removing that step also keeps lint independent of the
  installation dependency resolver. Directory/archive fixtures cover both cases.
- **Medium:** A successful lint exit does not establish valid metadata: upstream
  returns success after reporting validation findings. Hy now preserves that
  contract, while missing paths, unsupported suffixes and corrupt ZIPs still fail.
- **Low:** README checks follow file symlinks in source directories. Archive README
  naming also accepts a directory entry named README.md, matching upstream's name
  check; this does not relax referenced plugin-file validation.

- **Medium:** The shared RFC 3339 formatter hid distinct command contracts. API-key
  creation dates now use month/day/year and last-use values use relative text;
  shared-file creation includes seconds. Credential list columns now both fall
  back to raw text if either date fails, matching the source's single try block.
- **Medium:** Python's aware/naive distinction affects license expiration. A valid
  date-only string still falls back to raw text when compared with an aware UTC
  clock. Compact and week-date strings with offsets now receive relative text.
  Their API values remain strings and retain the existing lexical sort order.
- **Low:** CPython's C parser accepts two or more implicit fractional digits after
  the seconds in a basic time, while rejecting the equivalent extended time.
  The native parser preserves this observed distinction under A17. Explicit-clock
  key tests also preserve strict 60/3600 s thresholds and future-date day remainders.

- **Medium:** Re-running an extension's registration in a second execution process
  would repeat side effects, while parsing options to classify ownership could
  invoke callbacks twice. Discovery and invocation now share one interpreter;
  fixtures verify one registration and one parameter callback.
- **Medium:** Killing the interpreter when its control connection closed could
  hide a registration failure or its final output. Ordinary exceptions now use
  an explicit failure report, and early SystemExit waits for the interpreter's
  status before returning. Both zero and nonzero exit cases are covered.

- **High:** Registration cancellation and duplicate names previously risked changing
  the configured installation map. Cancellation now performs no commit and duplicate
  names retain their original paths; CLI fixtures compare the configuration bytes.
- **Medium:** Lexical version ordering can prefer 9.9 over 9.10. Instance removal
  now uses numeric components, with separate name tie-breakers for default selection
  and listing. This comparison depends on A6.
- **Medium:** The CLI default and idalib activation are independent configuration
  values. Switching to an installation without the native idalib library preserves
  the previous activation. A malformed idalib file still leaves the preceding CLI
  default change committed, matching upstream; the regression verifies this partial
  failure rather than implying a transaction across both files.

- **High:** The original Rust updater targeted the Python project's releases. It
  now derives its default repository from the Rust package metadata.
- **High:** Configuration fallback previously replaced malformed user data with
  an empty object. IDA plugin configuration now reports errors and preserves
  the original file. Global HCLI configuration is also loaded with errors
  propagated before command dispatch. Global setters now return persistence
  failures, and their callers propagate them before reporting success. Malformed
  credential records are also rejected instead of silently becoming empty defaults;
  this deliberately differs from upstream's fallback behavior.
- **Medium:** Root help omitted the local state needed to distinguish an absent
  installation, a stale default and unconfigured idalib. The status reader now
  evaluates these independently from filesystem/configuration snapshots; it
  neither authenticates nor persists migrations.
- **Medium:** Installation version detection inspected the binary before SDK
  metadata and could retain a trailing period from its docstring. A separate
  local reader now extracts the SDK major/minor version first, then falls back
  to the binary, directory and configured name. The normal current-installation
  API still honors its explicit environment override; help does not apply that
  override to unrelated configured installations.
- **Low:** Global quiet mode was advertised as suppressing prompts but had no
  implementation and no upstream counterpart. The flag is now rejected rather
  than silently accepted.
- **High:** Download cache reuse trusted Content-Length on HTTP error responses.
  A failed HEAD now falls through to GET. Fixtures return 403/500 with exactly the
  stale cache's length and verify fresh content is fetched.
- **High:** Streaming directly into the cache could destroy the last complete file
  on interruption. Cache publication now follows successful streaming into a
  temporary file on the same filesystem. A real short HTTP body verifies retained
  cache/output bytes and no staging remnants.
- **Medium:** API keys printed with leading slashes could turn cache paths into
  absolute paths. Leading slashes now remain relative to the download cache;
  parent traversal and backslash components are rejected. This is stricter than
  upstream's cache join and does not protect against existing parent symlinks.
- **Medium:** Basic regex matching rejected Python lookarounds and backreferences,
  and default matching was case-sensitive. The download matcher now accepts the
  exercised Python constructs and applies case-insensitive matching. Full Python
  regex compatibility is not inferred from these examples.
- **High:** License downloads used the displayed public ID in the API path instead
  of the license key. Requests now use the key after exact public-ID filtering;
  fixtures deliberately give these fields different values. The IDA installer
  fixture verifies the complete license acquisition and publication path.
- **Medium:** Interactive license downloads preselected every active license.
  The picker now starts empty, groups legacy before subscription records, and
  retains upstream's automatic selection only for a single matching record.
- **Medium:** Copying a license onto itself could truncate the source. Canonical
  paths and Unix device/inode identity now reject both direct and hard-link
  aliases before opening the destination. Windows hard-link identity remains
  unverified. Byte copying preserves access/modification times and permissions;
  extended-attribute copying is not implemented.
- **Medium:** Whole-file upload hashing allocated memory proportional to file size.
  Both upload commands now share a streaming implementation with an 8 KiB hash
  buffer. A file changed between hashing and transfer can still invalidate the
  checksum; the source is reopened for PUT as in upstream.
- **High:** A share download could replace an existing file without the upstream
  confirmation prompt. The target now uses the API filename and requires an
  overwrite decision unless forced. PTY fixtures verify cancellation preserves
  the original bytes and sends no content request.
- **Medium:** Missing upload fields could silently become an empty key or version
  zero. Required result fields now validate after transfer and confirmation,
  matching upstream's ordering. An invalid confirmation key fails after PUT;
  unsuccessful transfer or confirmation never produces the success message.
- **Medium:** Permission CSV parsing trimmed or dropped values, changing the
  request. Empty whole arguments are now omitted; whitespace and empty individual
  entries remain verbatim. Environment-key shares preserve upstream's async
  `api-key-user` placeholder: private permissions use that string and domain
  permissions use `@`. No actual email lookup is claimed for that path.
- **High:** Strict RFC 3339 decoding discarded credentials written with upstream's
  extra timestamp suffix. String-preserving decoding and ordered maps now retain
  those records and the upstream default-selection order.
- **High:** A stored interactive token previously counted as authenticated without
  a server check. GoTrue validation now precedes its use, including tokens with
  opaque payloads. OAuth no longer saves an unverified JWT email or fabricated
  one-hour expiry metadata. Credential writes follow successful user validation.
- **Medium:** `--auth` was parsed but ignored. The upstream constrained command set
  now rejects unsupported types, type mismatches and unavailable forced credentials
  before command side effects. Optional-auth and local commands retain their
  distinct upstream policies.
- **Medium:** Credential removal could leave a legacy refresh session behind.
  Associated session cleanup now commits atomically with removal; all-credential
  logout uses one commit rather than upstream's incremental per-record writes.
- **High:** OAuth used one 8 KiB socket read, so ordinary TCP fragmentation or
  larger tokens could fail login. HTTP framing now comes from Hyper, while the
  application bounds token data and connection lifetimes. Unknown routes drain
  bounded request bodies before closing; an observed reset during error responses
  was corrected and the complete suite rerun.
- **Medium:** Browser launch preceded callback listener binding, permitting a
  fast redirect to arrive before the listener existed. Binding now precedes launch
  and occupied-port errors are reported first. Callback JavaScript checks the HTTP
  status and displays receipt of the response rather than claiming credentials
  were already validated. Its actual browser execution remains unverified.
- **Medium:** A legacy shared session could refresh a different selected account
  and rewrite its email. Known email mismatches now fail before use or persistence
  (A5). Authenticated requests no longer fall back to expired tokens after refresh
  errors; the original credential/session file remains intact after failed writes.
- **High:** Dropping a temporary directory after failed plugin rollback could
  delete the backup named in the error. The staging owner is now retained.
- **High:** An editable source inside the replaced installation could be deleted
  when the old installation's backup is removed. Hy rejects this layout before
  publication and preserves both trees. This is stricter than upstream's direct
  replacement sequence. The regression covers a nested source directory.
- **Medium:** Upstream publishes an editable symlink before writing its `.pth`
  registration, so a registration failure can leave the replacement installed.
  Hy stages the registration first and restores the old installation if publication
  fails. This rollback extension does not undo preceding pip changes.
- **Medium:** A CR/LF-containing source path cannot be represented as one `.pth`
  path line. Hy rejects these paths before publication instead of writing extra
  lines. It also rejects non-UTF-8 registration paths explicitly.
- **Medium:** Malformed directories previously entered the managed inventory and
  could break dependency preflight or be advertised as installed. Canonical
  validation now precedes those operations. Unfiltered status retains upstream's
  separate minimal-descriptor and single-file legacy inventory.
- **Medium:** Matching two failed canonicalizations treated unrelated missing
  databases as identical. Exact matching now requires two successful resolutions.
- **Medium:** Setting validation now supports lookarounds and backreferences,
  with translated Python `$` and strict `\Z` semantics. The scanner preserves
  escaped literals, character classes and verbose/group comments. The 25 oracle
  cases do not establish full Python compatibility: ASCII/Unicode character
  classes, unsupported syntax and backtracking-limit behavior remain open.
- **High:** Ctrl-C during plugin configuration could terminate the process before
  installation cleanup. Setting prompts now return cancellation as an error;
  terminal regressions verify fresh-install removal, retained completed upgrades
  and unchanged configuration bytes after cancelling a partially answered form.
- **Medium:** Editable text was previously treated as a submission fallback,
  preventing users from clearing an existing value. Text now has an editable
  initial value. Accepting a descriptor default still preserves an existing
  override because upstream omits that answer rather than deleting the override.
- **Medium:** Host architecture does not identify the selected IDA architecture.
  Plugin installation, repository selection and bundle wheelhouse selection now
  use IDA's executable header; an unknown header produces an error. Doctor reports
  an unknown platform explicitly instead of substituting the host CPU.
- **Medium:** Bundle publication previously reused a predictable `.tmp.zip` path,
  which could overwrite an unrelated file. Publication now uses an exclusively
  created temporary file beside the destination and atomically replaces the
  destination only after ZIP completion. A regression preserves an existing
  `.tmp.zip` neighbor while successfully creating and installing the bundle.
- **Low:** Upstream search rendering indexes an empty IDA-version list and raises
  an exception. Hy renders `none`; compatibility still rejects every IDA version.
- **Medium:** Invalid multi-setting imports are validated completely before any
  configuration write. Upstream writes valid preceding keys before a later key
  fails. Hy intentionally preserves the original file on this failure path.
  Successful imports use the same descriptor validation and resulting settings.
- **Medium:** Invalid direct-archive descriptors were reported as missing because
  catalogue scanning discarded their errors. Direct selection now retains and
  reports validation diagnostics; catalogue scans continue to omit invalid
  descriptors. Regression tests verify failure before pip and publication across
  archive, directory and snapshot inputs.
- **Medium:** Upstream ZIP validation rejects explicit native filenames even when present; its final `has_bare_name` check requires an appended extension. Hy preserves this behavior. Directory validation accepts exact native filenames. Catalogue discovery now recognizes valid bare native entries instead of requiring a nonexistent bare file.
- **High:** The earlier updater could replace the executable with an HTTP error body or truncated download, used a temporary directory on a potentially different filesystem, and left downloaded files behind. Replacement now checks HTTP status and advertised size, ignores the asset name when constructing filesystem paths, and publishes a same-filesystem staged file. Failure fixtures verify unchanged executable bytes and no retained staging files on macOS. Windows rollback remains code-reviewed but unexecuted.
- **Low:** HTTP fixture listeners use nonblocking accept. Accepted connections now explicitly use blocking reads, preventing early close before request bytes arrive on hosts that inherit the listener's nonblocking mode.
- **Low:** Upstream refuses deletion of a required setting when its default is
  `false` or the empty string. Hy preserves this observed behavior; default lookup
  still returns those values correctly.
- **Medium:** Protocol registration previously had two different setup paths:
  ida protocol register omitted discovery, while ke setup rewrote registrations
  and selected a default alphabetically. They now share the upstream empty-registry
  policy and numeric default selection. The force flag does not overwrite instances.
- **Medium:** HashMap iteration made unscoped database lookup nondeterministic
  when multiple sources contained the same basename. Configuration mappings now
  retain insertion order; a fixture launch verifies the selected database path.
- **Medium:** GUI launchers lacked upstream's Python-environment reset and macOS
  diagnostic log. The native AppleScript fixture verifies both behaviors with
  quoted paths and URL metacharacters; live browser/OS registration remains open.
- **Low:** Source-name validation intentionally retains Python's single final-LF
  allowance. Tightening this grammar would change the pinned upstream behavior.
- **Medium:** Filesystem lookup previously used running-instance basename rules,
  so a request for sample.idb could select sample.i64. The lookup now uses the
  literal requested filename or its glob pattern. Extension-insensitive matching
  remains limited to running-instance selection, as upstream specifies.
- **Medium:** URL normalization changed ordinary filenames and navigation payloads.
  Ordinary link parsing now preserves raw percent escapes and forwards the original
  URI. Default-resource appending intentionally retains upstream's unusual placement
  after an empty query delimiter or fragment.
- **Medium:** Removing an invalid character-class range can expose a leading `!`
  that Python then interprets as negation. The initial native matcher missed this;
  the expanded oracle caught it and now verifies the corrected interpretation.
- **Medium:** Glob lookup can resolve a concrete filename different from the URI's
  pattern. Startup now matches that resolved name before forwarding the original
  navigation URI, avoiding a timeout while waiting for a literal wildcard name.

- **Medium:** A global version override could make a selected pre-9.4 installation
  enter an unsupported IPC wait. Launch now reads that installation's SDK/directory
  metadata independently; conflicting registered/environment selections have a
  fixture that verifies the actual executed binary and preserves configuration.
- **Medium:** The startup deadline previously also bounded auto-analysis. The two
  waits are now separate. Virtual-clock tests verify polling intervals and timeout
  edge values; a CLI test completes analysis after the startup limit has elapsed.
- **Medium:** A whole-exchange IPC deadline rejected slow but progressing replies.
  Per-read timeouts now match the Unix source contract. A real socket fixture sends
  three chunks over 9 s, exceeding the removed 7 s whole-exchange deadline.

- **Medium:** Querying every IPC candidate before checking a match could delay
  navigation behind unrelated endpoints. Enumeration and information queries are
  now separate; a socket fixture proves that the candidate following the selected
  instance receives no connection.
- **Low:** Unix stale-entry cleanup follows upstream's filename/PID policy, so a
  matching stale regular file can also be unlinked. Tests use an isolated directory;
  cleanup failures do not abort enumeration. PID reuse and concurrent replacement
  remain outside the fixture's guarantees.

- **Medium:** Sidecar failures previously aborted KE downloads, and arbitrary bytes
  could be saved as metadata. Metadata now runs first, validates encoding and JSON,
  and warns on failure while permitting the separately verified content transfer.
  The invalid-UTF-8 regression caught a parser path that checked syntax but skipped
  string encoding; explicit decoding now precedes syntax validation.
- **Medium:** Deserializing sidecars into serde_json::Value rejected Python-accepted
  byte encodings, unpaired surrogates, non-finite values, large numbers and ordinary
  documents beyond serde's recursion limit. Dedicated decoding/token adapters now
  validate these cases while the transfer retains the exact original bytes.
- **Low:** Constant adaptation must preserve lexical boundaries: replacing NaN inside
  a number or quoted string could change acceptance. A 13,107-case source-oracle
  digest exercises malformed token combinations; separate cases cover escaped quotes.
- **Low:** Native metadata syntax validation uses heap state proportional to nesting
  instead of interpreter recursion. It therefore does not reproduce Python's
  stack-dependent RecursionError, including that exception's transfer-aborting behavior.
- **Medium:** The previous 300 s whole-request deadline could interrupt a progressing
  KE transfer. Separate connect/read timeouts now follow upstream's inactivity model;
  a loopback response takes over 1 s with a 600 ms read timeout and completes.
- **Medium:** Passing all pins to the HTTP connector delegated candidate order and
  retry decisions to its connection strategy. One client per address now makes order
  explicit, retries only connection failures and preserves the original Host header.
  Header/body timeout and HTTP-error fixtures prove that those failures do not fail over.
- **Medium:** The old IP classifier missed CPython 3.13 private IPv6 ranges and rejected
  documented public exceptions. The pinned source oracle now covers registry boundaries.
  Upstream also permits deprecated fec0::/10 site-local addresses; that exception is
  preserved and recorded rather than described as a general public-address guarantee.
- **Low:** A client with a DNS override can otherwise be reused with an unrelated host
  and resolve it normally. Prepared KE transports now retain their origin and reject
  cross-origin use before making any request. Redirects obtain a new prepared transport.
- **Medium:** Compressed responses were previously hashed and saved as wire bytes.
  Content decoding now precedes size accounting, metadata validation and hashing.
  A fixture serves an incompressible 1 MiB IDB whose gzip body exceeds 1 MiB; a 1 MiB
  configured limit now accepts it. Separate fixtures reject decoded expansion beyond
  the content or 16 MiB metadata limit while preserving existing files.
- **Medium:** Raw-deflate fallback is sensitive to the first nonempty network chunk.
  The source oracle caught an empty-chunk mismatch, now corrected. Empty intermediate
  output in a stacked encoding still reaches the next decoder, matching HTTPX.
- **Low:** HTTPX does not require gzip trailers to be present and ignores subsequent
  gzip members after the first. Native tests preserve these rules while rejecting
  a supplied invalid checksum; hash verification still checks the resulting IDB bytes.
- **Medium:** Metadata responses can establish or update the session required for
  content, including on HTTP errors and redirects. The shared jar now preserves
  those updates; command fixtures reject content requests lacking the expected cookie.
- **Medium:** IP pinning changes cookie identity in upstream: two hostnames using
  the same IP can share a host-only cookie within one transfer. A socket fixture
  verifies this behavior while preserving each request's original Host header.
- **Low:** CookieJar parses an entire response before inserting its cookies. An
  expired Set-Cookie deletes a previously stored value but does not remove a value
  awaiting insertion from the same response. The HTTPX oracle exposed this ordering
  difference; the native jar now preserves it and tests both update and deletion cases.
- **Medium:** A generic cookie date parser applies different century and historical
  date rules from Python. Native expiry parsing now follows the pinned source. A
  strict pre-1970 date is ignored, leaving a session cookie; loose short years use
  the closest century with an exact 50-year boundary. CLI fixtures cover both.
- **Low:** An invalid strict month can raise during Python's date parsing and cancel
  every cookie in that response, including an earlier pending expiry deletion.
  Parsing all date attributes before mutating the jar preserves this behavior, even
  when Max-Age or an earlier Expires attribute would otherwise take precedence.
- **Medium:** Saturating cookie integers to 64 bits loses Python's distinctions
  between a future expiry, binary64 rounding and conversion overflow. Native storage
  now retains arbitrary precision and follows the constructor conversion. Tests cover
  expiry at 2^53 s and the binary64 overflow boundary 2^1024 - 2^970 s.
- **Low:** Missing Max-Age and nonnumeric Max-Age do not have the same batch effect.
  A missing value cancels normalization before any deletion; nonnumeric text rejects
  one cookie. Construction overflow cancels pending insertions after earlier deletions.
  Separate normalization and construction stages now preserve those outcomes.
- **Medium:** Python accepts negative cookie versions and skips their source-domain
  suffix check. A Unicode negative-version fixture can set a cookie for another
  dotted domain; native policy now preserves that observed behavior. Negative-version
  return checks also preserve Python's distinction between localhost and localhost.local.
- **Medium:** Ignoring Set-Cookie2 dropped cookies accepted by upstream when an
  ordinary Set-Cookie header was also present. Extraction now preserves both batches,
  including empty ordinary headers, duplicate replacement and failures in either format.
  A CLI fixture requires the legacy cookie to authorize content and checks that an
  ordinary cookie with a longer path is sent before it.
- **Low:** An HTTP fixture worker assertion followed by a main-test assertion caused
  a second panic during Drop, aborting the suite and skipping remaining cleanup.
  The fixture now joins its worker without raising another panic while unwinding.
  A regression deliberately triggers both assertions and verifies the original panic.
- **Medium:** The prior proxy review assumed original-host TLS identity through a
  CONNECT tunnel. A read-only HTTPcore backend probe contradicted that assumption:
  the inner TLS identity is the rewritten IP; only an HTTPS proxy's outer connection
  receives the original SNI extension. Native proxy connections now reproduce those
  identities. A generated DNS-only certificate is rejected for the pinned IP tunnel.
- **Medium:** Applying bypass rules to the original hostname would disagree with
  HTTPX after pinning. Native selection uses the rewritten IP URL. Owned sockets show
  that NO_PROXY=original.test still selects the proxy for a pinned loopback address,
  while NO_PROXY=127.0.0.1 bypasses it.
- **Low:** Proxy and origin Basic authentication occupy separate headers. The CLI
  fixture verifies both credentials and metadata cookies, while TLS fixtures verify
  that Proxy-Authorization appears on CONNECT but not on the tunneled GET.
- **Medium:** Native proxy requests currently open separate connections rather than
  sharing HTTPX's connection pool. SSL_CERT_DIR, certifi/system
  trust-store equivalence and optional SOCKS transports remain implementation gaps.
- **High:** SSL_CERT_FILE previously configured only proxy connections. Direct KE
  requests ignored the override. Session-owned origin TLS now applies to direct
  requests and CONNECT tunnels. The CLI accepts its fixture root, rejects an
  unrelated root, and preserves both existing files on rejection; HTTPX agrees for
  those same certificate files. The original-host check remains active under pinning.
- **Medium:** The environment-key identity fallback depends on upstream command
  execution context. Resolving every status remotely would add requests to async
  installation, switching and sharing. Native synchronous status now resolves
  identity explicitly, while those async flows retain their placeholder behavior.
- **High:** Reusing standalone key validation for status exposed its prior automatic
  redirect handling. Identity requests now preserve upstream's no-follow behavior.
  An owned second server receives no request, including when a 302 response contains
  valid email JSON; a reflected API key in an error message is not printed by status.
- **High:** ASCII-only numeric settings treated a Unicode positive download cap as
  invalid and therefore unlimited. KE settings now use the existing Unicode integer
  parser. CLI fixtures reject 1,048,577-byte content for Arabic-Indic or Devanagari
  one-MiB limits and preserve the cached IDB.
- **Medium:** Casting retention days before multiplication can round differently
  from Python's integer-first calculation. Values outside i64 also previously fell
  back to three days. Native cleanup now preserves the parsed integer, rounds after
  multiplication, and returns conversion overflow before any cache mutations when
  the root exists. The upstream early return for a missing root is also retained.
- **Medium:** Sharing one TLS context across an HTTPS proxy and its tunneled origin
  conflated their trust sources. They now have distinct contexts; independent
  certificate fixtures reject the outer or inner connection when only that layer's
  roots are wrong. Complete platform/default-root equivalence remains open under A15.
- **Medium:** An unrelated environment proxy entry suppresses OS fallback upstream.
  Filtering unknown schemes before discovery would change routing. Nine fixtures
  verify the lazy fallback boundary, including lowercase deletions and CGI removal.
  CLI fixtures supply an unused proxy scheme to isolate them from machine settings.
- **Low:** A malformed Windows ProxyServer fragment stops parsing but retains earlier
  entries and skips subsequent SOCKS fallback. The parser preserves this behavior;
  the read-only CPython comparison includes trailing separators and malformed schemes.
- **High:** A small encoded chunk can expand substantially before final-layer limits
  apply, especially through stacked encodings. The decoder checks final output in
  64 KiB increments before appending it, but intermediate buffers and unlimited
  content retain no independent memory cap. This remains a resource-limit gap.
- **Medium:** Rejecting every symlink ancestor made KE downloads fail under normal
  macOS temporary paths and through internal cache aliases. Resolved containment now
  allows both while rejecting targets outside the configured root. Validation precedes
  any request; fixtures verify internal downloads and unchanged outside targets.
- **Medium:** A sidecar symlink can direct upstream's metadata write outside the cache.
  Hy's atomic publication replaces the link. A fixture verifies that both a content
  link's internal target and a sidecar link's outside target retain their original bytes.
- **Low:** Resolved containment does not lock the directory tree. A concurrent actor
  replacing a parent link after validation can change the eventual write destination.
  Neither the pinned handler nor this implementation closes that race.
- **Low:** Sidecar and content publication are separate operations, as upstream
  specifies. A valid new sidecar can coexist with the old IDB after a content hash
  failure; the regression verifies this state explicitly.

- **Medium:** A terminal-only confirmation could not serve browser-launched KE
  requests. Native command builders now provide confirmation, progress and failure
  dialogs, and subprocess fixtures prove that rejection makes no download request
  and preserves even cache files that negative retention would otherwise expire.
- **Medium:** KE environment values previously recognized only literal 1, and
  malformed download limits aborted the command. Startup settings now accept the
  upstream boolean spellings and normal integer syntax/defaults, subject to A11.
- **Medium:** Retention cleanup was missing. It now follows host validation and
  confirmation, so a rejected host or declined dialog cannot trigger deletion.
  The initial validated client is reused for both metadata and content requests.
- **Low:** Retention removes empty directories regardless of their age. A fixture
  for sidecar-publication failure must therefore keep its blocking directory
  nonempty; the full workflow test caught and corrected that fixture assumption.

- **Medium:** Rebuilding KE queries through a URL serializer changed percent escapes,
  plus signs and bare flags. A dedicated parser now removes only literal url fields
  and forwards the remaining query spelling; the source oracle includes encoded keys,
  control-character cleanup, invalid textual ports and fragments.
- **Medium:** Open-only KE links previously sent a navigation command. Optional
  navigation is now explicit in the target type. Tests cover an existing database
  and a newly launched SDK 9.4 fixture that still completes startup and auto-analysis.
- **Medium:** Browser-originated launch failures lacked the upstream native error
  callback. A distinct launch error variant now marks installation, process and
  startup failures; IPC navigation rejections do not trigger that callback.
- **Low:** Upstream decodes keys when choosing the final download URL but removes
  only literal url keys when relaying the URI. Encoded aliases can remain in the
  forwarded query; this asymmetry is retained and covered by the oracle.

## Validation and quality gates

Run from the Rust project root:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
git diff --check
```

Regression tests exercise isolated CLI operations and native protocol compilation.
The latest uninterrupted serial all-target run passed 275 unit tests and 444
integration tests on macOS: 719 passed, no failures. A72 records this validation;
A59 retains the history of its earlier interrupted runs and fixture corrections.
Clippy warnings are treated as errors. Rustfmt, whitespace checks and Windows
cross-compilation also pass.
This run enabled the read-only Python source oracles, including packaging 26.0
for bundle downloads and target selection. Bundle descriptor comparisons also use
the extension runtime described under A45/A46. Timestamp field comparisons use
the locked Pydantic runtime under A47. Bundle ZIP comparisons use the reader and
toolchain configuration documented under A48. Local bundle path comparisons use
the shared-path and read-policy fixtures under A49. Repository packaging comparisons
use the source loop and transport adapters under A50. Reference parsing and bundle
preprocessing use the source comparisons under A51. Snapshot validation and export
use the envelope and CLI comparisons under A52, with snapshot text comparisons
under A53. Repository selection and installation acquisition use A54's actual
source functions; bundle catalogue ordering and equality use A55's comparisons.
Shared archive acquisition uses A56's source comparisons and native lifetime tests.
Ordered ZIP names and named reads use A57's directory, codec and consumer comparisons.
Wheelhouse extraction uses A58's source comparisons and retained-file regression.
Installation selection and extraction use A59's source comparisons and CLI regressions.
Lint archive discovery and reporting use A60's source-function comparisons; the
existing command-level lint oracle was also enabled.
Reference grammar, lexical joins and real directory/dependency lookups use A61's
read-only source comparisons. Directory packing and retained installation sources
use A62's comparisons and regressions. The configured-environment extension suite
was rerun separately under A62 with all nine tests enabled; all nine passed.
Direct acquisition branch selection and GitHub release parsing use A63's source
comparisons and owned-server CLI regressions.
File-URL decoding, archive reads and repository construction use A64's conversion
and source-consumer comparisons.
Repository redirect, credential and response policy uses A65's actual-source
comparisons and production-client wire regressions.
Direct GitHub release selection and acquisition use A66's source-function,
HTTPX-transition and owned-server installation comparisons.
Release JSON values and byte encodings use A67's CPython decoder projections,
expanded source-selection corpus and installation regressions.
Automatic redirect targets use A68's HTTPX target-construction comparisons,
expanded GitHub request-policy corpus and API/GitHub wire regressions.
Catalogue retries use A69's actual Tenacity-decorated source function, scheduler
regression and CLI acquisition fixtures.
Catalogue archive planning and cache identities use A70's source getter-call
projections and command-level acquisition/order/cache regressions.
Catalogue GraphQL batching uses A71's actual-client query/envelope comparisons
and command-level batch/cache-publication regressions.
Catalogue cache reads use A72's source-getter filesystem projections and CLI
failure/expiry regressions.
The earlier A54 parallel run observed an OAuth callback shutdown
assertion failure at `src/auth/oauth_tests.rs:158`; that assertion passed in the
serial run. Its intermittent cause is unknown; port reuse is an unverified
hypothesis. No OAuth code or assertion was changed for that failure. A59 separately
corrects the test client's HTTP response boundary after a different reset failure.
Nine opt-in tests remained ignored
across 62 test suites.
Seven opt-in Python-extension integration tests also passed using the runtime
documented in extensions.md; these require HY_TEST_EXTENSION_PYTHON and are ignored
by the default suite. The release-profile background update test also passed and
is ignored in the default debug suite. One additional manual browser test remains
ignored and has not passed. Ruff formatting and lint checks pass for the bridge and its fixture.
`RUSTFLAGS='-D warnings' cargo check --offline --locked --all-targets --target x86_64-pc-windows-gnu`
also passes with warnings denied; this checks Windows-specific executable and test code, not Windows
runtime behavior. The Linux ARM64 check
stops in native dependency compilation because `aarch64-linux-gnu-gcc` is absent.
They do not certify live authentication, installer execution, registered protocol
handling, all operating systems, or all upstream requirements.

Shared-file selection validation enumerates all edit sequences of lengths zero
through four over T, Z, Backspace, next, previous, toggle, toggle-all and invert:
1 + 8 + 64 + 512 + 4,096 = 4,681 sequences. Each result records visible indices,
selected indices, cursor, query and whether the current query matches. The installed
questionary InquirerControl provides the source oracle; checkbox toggle operations
follow its checked-in bindings. The compact JSON result vector has SHA-256
`fceae4acf0e35442c1e0e5745c58387ea2bf88f026a6a5e4970d6a48d547a324`,
which is asserted by the default suite. Repeating the same corpus with equal
values in the first two rows produces
`08372d5e41d7622a3a7b9336ec675c5b2455a7931a101685042ef74be8ed645c`.
This second digest is also pinned. A separate state test retains selections
across longer filters. Seven terminal selection scenarios and five cancellation/
empty-selection scenarios verify API requests. A thirteenth CLI scenario forwards
negative limit/offset arguments. Existing share download/batch fixtures also pass
with the output directory's editable ./ initial text.

Two additional terminal scenarios compare equal records whose nested numeric
metadata differs only as true versus 1.0; a third record has the same visible label
but unequal metadata. Selecting one equal value submits both records. Inverting
then toggling once preserves the remaining equal occurrence. The fixture also
checks an arbitrary-precision version in noninteractive output and coerced paging
fields. Five malformed-response scenarios reject explicit null size, version,
offset, limit and total before management.

Asset integer validation covers 37,498 raw JSON inputs, including numeric strings,
booleans, signed values, float limits, leading-zero/underscore cleanup and 4,300-digit
boundaries. Its compact normalized result vector has SHA-256
`7c78a33d2a4170db1f7c839d0d417f4163d254efb2ac965145172988f0ba0924`.
The source Asset class supplies 123 field/default/required-key comparisons. Eleven
additional metadata equality pairs check container shape and exact integer/float
comparison around 2^53 and 2^64. Nineteen source formatter comparisons cover zero,
negative sizes and boundaries around successive powers of 1,024 bytes through PiB.
The upstream labels remain B/KB/MB/GB/TB despite the binary scale. Invalid sizes
fail in native listing instead of saturating the unit to TB; exact exception text
is not compared.

The Asset oracle uses an isolated Pydantic 2.12.5 / core 2.41.5 installation matching
upstream's lockfile. The existing extension runtime actually contains Pydantic
2.13.5 / core 2.46.5; its use for other source-function comparisons does not certify
locked model behavior. Primary parser sources are upstream `src/hcli/lib/api/asset.py`,
[pydantic-core 2.41.5 shared input parsing](https://raw.githubusercontent.com/pydantic/pydantic-core/v2.41.5/src/input/shared.rs)
and [jiter 0.12.0 number decoding](https://raw.githubusercontent.com/pydantic/jiter/v0.12.0/crates/jiter/src/number_decoder.rs).

```sh
HY_TEST_ASSET_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy asset_models
HY_TEST_ASSET_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy size_units
```

Bounded findings: **high impact**—changing the Pydantic runtime changes the accepted
numeric domain, so the oracle checks its exact version. **Medium impact**—global
serde_json arbitrary-precision support affects all JSON consumers and requires the
complete regression suite. **Medium impact**—equal duplicate assets can cause
repeated API actions under upstream's selection semantics; owned HTTP fixtures
verify that behavior without operating on a live account.

Primary sources are `src/hcli/commands/share/list.py`,
`src/hcli/commands/common.py::safe_ask_async` and the installed questionary 2.1.1
`prompts/checkbox.py`/`prompts/common.py` selection and filter implementations.

Standalone share validation additionally covers `src/hcli/commands/share/get.py`
and `delete.py`, executed from the pinned checkout with their decorators removed
and API calls replaced by in-memory adapters. Four output-destination cases, one
forced deletion, two missing-URL cases and six invalid-size/model cases supply
thirteen comparisons. Successful plain-text reports are compared exactly; failed
cases compare status, lookup/download/delete counts and whether the completion
message preceded the failure. Error-detail wording is not asserted equal.
The native fixtures verify actual downloaded bytes, retained files after reporting
failure, absence of forced DELETE on invalid sizes, and null rejection before
transfer. A separate conflicting-options fixture checks stdout, stderr, status 1
and zero API requests.

Confirmation tests compare thirty-six strings against locked Click confirm and
Rich Confirm.process_response, including capitalization, full words, empty input,
Unicode whitespace and invalid answers. Eight redirected-input CLI scenarios
cover acceptance, rejection, retry and EOF. Existing PTY fixtures still cover
overwrite and deletion confirmation. Canonical terminal Ctrl-C still uses the
native signal behavior and is not claimed equivalent to Python's Abort handling.
Path resolution reuses the existing platform implementation, now in util/realpath;
four Unix tests revalidated its seven read-only CPython comparisons after the move.
Windows unresolved-reparse behavior retains the documented stricter boundary.

```sh
HY_TEST_SHARE_REPORT_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test share_operations
HY_TEST_SHARE_REPORT_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy confirmation_grammars
HY_TEST_PATH_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy paths::tests
```

Bounded findings: **high impact**—upstream can fail a size report after a download
has already been published; the native command now preserves that ordering.
**Medium impact**—an invalid deletion size prevents its API mutation even with
force. **Medium impact**—resolving output links changes both the reported path and
the interpretation of parent components after symlinks; the resolver is shared
with cache containment rather than independently reimplemented.

```sh
HY_TEST_SHARE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy share_list
cargo test --offline --locked --test share_selection --test share_operations
```

MCP validation uses six unit tests and four CLI tests. Forty setup scenarios cover
the five agents, their eight supported agent/scope combinations and installed,
marketplace-only, absent, failed-query and malformed-listing states. Five additional
upstream comparisons verify checked-command failure stops. The recorder compares
the complete query/mutation sequence and success status with the installed Python
functions. Native tests also cover missing executables and SIGTERM status -15.
An exhaustive reference scan over non-ASCII Unicode scalars verifies the eleven
ASCII casefold mappings used for fixed-name detection. Thirteen CLI scenarios use
local release responses, a controlling terminal and isolated executable shims;
they cover all agent/scope combinations, Unix `.cmd` fallback, cancellations,
missing agents, checked-command failure and plugin-download failure.

```sh
HY_TEST_MCP_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy cmd::mcp
cargo test --offline --locked --test mcp_install
```

Update validation compares fourteen release selections with the installed upstream
implementation, including drafts, equal-precedence build tags, development markers,
force thresholds and Python tag whitespace. Sixteen cache inputs compare strict
24 h boundaries, future/naive/aware timestamps and malformed values using a fixed
clock. Worker tests cover timeout followed by completion, duplicate-start prevention,
preserved stale cache after failure and advisory cache-write failure. The opt-in
release-profile CLI fixture uses owned HTTP listeners to verify stable/development
selection across pages, no-update notifications, failed-check silence, application
cache paths and fresh-cache suppression. Replacement fixtures use isolated copies;
they reject HTTP 201/206 as well as empty, short, long and error responses.

```sh
HY_TEST_UPDATE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy update::
cargo test --offline --locked --test self_update
cargo test --release --offline --locked --test background_update -- --ignored
```

Lint validation runs four CLI tests. The optional read-only CPython oracle invokes
the installed upstream Click command for seven scenarios in both directory and ZIP
form, three failing input cases and two home-relative paths: 19 invocations total.
All statuses are compared; ten valid-source outputs are compared byte-for-byte with
upstream Rich color disabled and width set to 10,000 columns. This display assumption
is falsified by colored/narrow terminals and does not certify Rich layout equivalence.
Validation-error text is intentionally outside that comparison because serde and
Pydantic expose different error structures. Both dependency-file cases remain valid.
The default suite enforces the native outcomes without requiring an interpreter.

```sh
HY_TEST_LINT_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_lint
```

Authentication status fixtures exercise twelve API response variants, including
valid/empty email strings, malformed models/JSON and HTTP 401/403/404/429/500.
Separate fixtures prove no-follow redirect handling, invalid-header fallback before
network access, unchanged managed credentials, post-commit default identity lookup,
and the async installation placeholder. All requests target owned local listeners.

```sh
cargo test --offline --locked --test auth_status
```

Datetime validation compares 9,629 strings with CPython, retaining exact calendar
and time components, microseconds and optional UTC offsets. The compact result
vector SHA-256 is
`573de527516592a60766c06cad6e7cfb80c110f47030b6cc71c45e85b64c6548`;
this assertion also runs without Python. The corpus covers basic/extended and week
dates, invalid calendar values, Unicode/numeric/NUL separators, fractional values,
implicit basic-time fractions and invalid/mixed offsets. Fixed-clock formatters
compare 45 license labels and 21 API-key date pairs with the upstream functions.
Three CLI fixtures cover credential fallback, API-key columns, shared-file seconds,
and aware versus naive license expiration.

```sh
HY_TEST_DATETIME_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy iso_datetime -- --nocapture
HY_TEST_DATETIME_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy expiration -- --nocapture
HY_TEST_DATETIME_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy auth_cmd::dates -- --nocapture
cargo test --offline --locked --test date_reports
```

Link startup tests use executable shell fixtures and owned Unix sockets. They
verify registered-default precedence over environment selection, SDK version
precedence over the global override, actual Unix session detachment and suppression
of launcher stdout/stderr. A PTY test sends Ctrl-C after the first analysis response
and observes successful navigation. An analysis fixture takes at least 5 s despite
a 1 s startup timeout. Separate virtual-clock tests verify the 0.1 s initial poll,
1.5 multiplier, 2 s cap and deadline overshoot between polling cycles. Zero,
negative and NaN startup timeouts perform no polling after launch; positive infinity
can continue indefinitely. Existing-instance navigation ignores all these timeout
values. No real IDA binary is launched by these fixtures. Accepted fixture sockets are
explicitly returned to blocking mode before applying their read deadline; this
removes a macOS nonblocking-read race caught by the full suite.

KE tests send requests only to a loopback fixture with the private-host override
explicitly enabled and downloads confined to a temporary home. They assert metadata
before content, capability-token preservation, byte-for-byte valid sidecars and old
file retention after invalid UTF-8/JSON, oversized metadata, failed sidecar writes,
truncated UTF-16, over-limit integers, non-200 content, hash mismatch and an exceeded
content limit. Failed transfers leave
no staging files. Unix fixtures allow a symlinked configured root and an internal
directory alias, reject an outside destination before HTTP, and replace file links
without modifying their targets. Three CLI filenames cover colons, invalid UTF-8
replacement and a C1 character. No real IDA binary or native dialog tool is executed.
Unix fixture tools record literal dialog text and run a disposable sleep process
for progress. Tests verify that process is reaped after success or failure. Approval
tests execute only an isolated shell launcher with SDK 9.3 fixture metadata.
Windows transfer tests substitute an invalid powershell.exe to avoid real UI.
Separate command-construction tests cover macOS, Linux and Windows arguments.
Transport fixtures use owned loopback sockets and private test constructors; no public
host is contacted. A closed endpoint followed by a live endpoint verifies ordered
failover and the original Host header. A 503 response, a stalled header and a stalled
body do not reach the second candidate. Four body chunks arrive at 250 ms intervals
under a 600 ms read timeout; total transfer time exceeds that timeout. Redirect cases
cover all five statuses, relative queries, absent Location, five/six-hop boundaries,
blocked target rejection and non-HTTP schemes. HTTPS-to-HTTP target selection is a
pure response-policy test, not a TLS integration test. Pinned and unpinned requests
verify the distinct userinfo policies. An origin-mismatch fixture sends no request.
Nine additional metadata payloads cover BOM-marked UTF-8/16/32, Python constants,
a raw surrogate, a 4300-digit integer and 200-level nesting. Every accepted sidecar
retains the exact decoded payload bytes. Metadata unit tests compare 13,509 outcomes with
CPython, including a fixed token-corpus digest that also runs without Python:

```sh
HY_TEST_JSON_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy ida::links::metadata -- --nocapture
```

Proxy selection tests compare 186 cases with HTTPX, including environment casing,
CGI behavior, scheme priority, domain boundaries, exact localhost/IP matching,
IP-prefix-shaped strings, ports, uppercase schemes and wildcard bypass. The compact
serde_json result-vector digest is
`707d600fd18ad33e72b00b0a98740ba46bf939ef27fae6079948dceb650a34f3`.
Eight real TLS modes exercise the actual Transport with pinned and unpinned URLs.
Other fixtures distinguish connection refusal from header inactivity timeout and
verify read/write timeout resets using a virtual clock. The proxy response owns its
connection tasks, which are aborted when the body is released. CLI fixtures clear
inherited proxy/CA variables and set only their own loopback endpoints. An unused
proxy scheme suppresses ambient OS discovery without creating an HTTP route.
Nine environment cases check lazy fallback, and a typed Core Foundation fixture
checks enabled, disabled, missing-host and optional-port settings. The optional
oracle compares the current macOS snapshot and executes the installed CPython
Windows parser against 132 synthetic registry values. It does not write system
settings. Native Windows registry access has only been cross-compiled.

TLS configuration tests cover twelve CA-file inputs against CPython, including
duplicate certificates, leading/trailing text, CRLF, a missing final newline,
an unrelated private key and malformed certificate data. Three direct TLS modes
exercise configured trust and original-host checks through the actual Transport.
Three separate proxy/origin trust combinations exercise CONNECT. A CLI fixture
performs trusted and untrusted downloads, then asks HTTPX to use the same files
against the same owned TLS listener. No certificate verification is disabled.

KE settings compare 78 raw environment values with the installed upstream reader,
including Unicode digits, information-separator whitespace, malformed signs and
underscores, large signed values and the 4300-digit boundary. Retention compares
113 exact binary64 cutoff results or conversion failures with CPython at integer
precision and float-overflow boundaries. The compact result-vector SHA-256 is
`df5336d11dbc880e6c7856a80ecc4849e45258bc15ccc88ff38f13cf6ba178d6`;
the digest also runs without a Python oracle. CLI tests verify Unicode one-MiB caps,
large positive/negative retention, defaults, and missing/existing-root overflow.

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy config::ke -- --nocapture
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy retention -- --nocapture
```

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy ida::links::transport -- --nocapture
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test ke_downloads tls_tests -- --nocapture
```

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy ida::links::transport::proxy -- --nocapture
```

Content-decoding tests use Rust-generated gzip/zlib/raw-deflate bytes and compare
HTTPX's output for identical chunk boundaries. The 31 source-oracle cases cover
fragmentation, repeated/stacked/unknown encodings, Latin-1 header whitespace, empty
chunks, first-call fallback, truncated trailers, concatenated members and corruption.
Independent limit tests cover cumulative output and highly compressed expansion.
Four CLI fixtures verify decoded metadata/content publication, gzip/zlib/raw/stacked
encodings, request negotiation, old-file preservation and encoded-size independence.
The new zlib-rs backend also participates in the full archive/plugin suite because
Cargo unifies flate2 features across the dependency graph. To repeat the HTTPX oracle:

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy util::http_body -- --nocapture
```

The decoder now lives in `src/util/http_body.rs`; API response ownership and chunk
consumption live in `src/api/response.rs`. Explicit accessors expose status and
headers without exposing an alternative undecoded body reader. API client creation
is shared by ordinary requests and standalone key validation. No codec dependency
was added for API support.

Six API compression tests exercise 64 isolated CLI invocations: 15 JSON-method/
UTF-16 combinations, ten identity/error cases, ten downloads across five encoding
profiles, one corrupt-cache preservation case, twenty buffered/streamed HTTP errors,
and eight PUT final/redirect cases. The error-order oracle executes the pinned
`APIClient.get_json` and `_handle_response` method bodies with HTTPX 0.28.1. Its
custom AsyncByteStream remains unread until the client requests it; constructing
an already-consumed response would invalidate the streamed-error comparison.
The existing 31 decoder comparisons were rerun after moving the implementation.

Primary sources are `src/hcli/lib/api/common.py::{get_json,put_file,download_file,
_handle_response}` and HTTPX 0.28.1 `_client.py::AsyncClient.send`,
`_models.py::Response.aiter_bytes`, and `_decoders.py::SUPPORTED_DECODERS`.
The source tree remained clean at the pinned revision during this verification.

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test api_compression
```

Bounded findings: **high impact**—a corrupt compressed response can prevent an
upload confirmation even when its HTTP status is successful. **Medium impact**—
streamed download errors cannot obtain the message from an unread JSON body.
**Low impact**—comparing encoded HEAD length with decoded cache size can trigger
another download. All three behaviors are retained from upstream; staged native
cache publication additionally preserves the previous artifact on decode failure.

The cookie engine and HTTPX header decoder now live in `util/cookies` and
`util/http_headers`, shared by API and KE transports. API requests use one
process-local jar across newly constructed clients and clones; KE retains its
existing independent per-transfer jars. Cookie headers are selected before each
request and response cookies are extracted before HTTP status classification or
body decoding. Redirects rebuild their Cookie header from the updated jar. API
Debug formatting excludes cookie contents.

Four API integration tests exercise seven process invocations: two signed-upload
host/scope cases, two fresh-process redirected downloads, two failed-response
batch cases, and one unencodable-cookie failure. They verify path ordering, Secure
and domain exclusion, same-host cross-port sharing, response updates, Max-Age
deletion, and failure before a request carrying a non-ASCII Cookie header. A
four-request socket test verifies that new API clients and clones share updates
and deletions. Three read-only HTTPX comparisons verify Location decoding under
UTF-8, Latin-1 and a mixed-header fallback, including inherited fragments.

Primary sources are `src/hcli/lib/api/common.py::{_api_client,get_api_client}` and
HTTPX 0.28.1 `_client.py::_send_single_request`, `Cookies.extract_cookies`,
`Cookies.set_cookie_header`, and `Headers.encoding`. The existing cookie/date/
tokenizer corpora were rerun after moving the implementation. API session mutexes
cover only header selection/extraction and are released before network awaits;
concurrent request scheduling and arbitrary client substitutions remain unverified.

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy cookies -- --nocapture
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy location_decoding
cargo test --offline --locked --test api_cookies
HY_TEST_SHARE_REPORT_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test share_operations
```

Bounded findings: **high impact**—failed HTTP or JSON responses can still change
the cookie state used by a later operation. **High impact**—an exception catch in
a wrapper does not establish that its actual dependency raises that exception;
the shared-file lookup audit now checks both layers. **Medium impact**—cookies
can cross ports on the same host while remaining excluded from a different host.
**Medium impact**—one non-UTF-8 header changes decoding of other header values
under HTTPX's response-wide encoding choice.

Cookie policy tests compare 144 cases with the same read-only HTTPX runtime: 35
domain/path/Secure/Port cases, 22 expiry/attribute cases, five ordering cases, eleven
legacy-date/batch-failure cases, 23 Unicode/large-integer cases, twelve normalization/
construction cases, seven binary64 rounding/overflow cases, 21 legacy selection/quoting
cases and eight mixed-format batch-ordering cases.
The oracle fixes Unix time at 1,800,000,000 s and reports the resulting Cookie header
or header-construction failure. No Python process edits files. Four socket fixtures
check independent sessions, shared redirect state, HTTP-error cookies and IP-based
scope across hostnames. Four CLI fixtures verify metadata-to-content propagation,
including 404 metadata and a redirect that updates the original transport's jar.
They also check Unicode and above-i64 Max-Age values, and preservation of the cached
IDB after missing-Max-Age or overflow cancels a session required by content requests.
Mixed-header CLI variants cover absent, empty, valid and malformed Set-Cookie alongside
Set-Cookie2. Legacy-only headers are ignored; a malformed Netscape batch does not cancel
the legacy cookie that authorizes content. Path length takes priority over format order.
The metadata fixture also requires cookies with a rolling short-year expiry and an
ignored pre-1970 expiry. Its short year is derived from the runtime's local year;
a hard-coded year crossed Python's 50-year boundary between the fixed oracle clock
and the actual CLI clock, which the first command run exposed and the fixture corrects.
Date tests compare 7,541 results against CPython's http2time with explicit local-year
values. They include years 0 through 1000 at five current-year settings, month/day
boundaries, named/numeric timezones, optional clocks, overflow, malformed text and
4300/4301-digit year limits. The native corpus runs without Python using the source-
verified SHA-256 `0ad4aef99076e4553bff6257dcd3ea5b922e55f5b082113aebcc5ee5913e1fc4`
over its compact serde_json result vector. Fixed timestamp and century-boundary
assertions supplement that digest.

Legacy header tokenization compares 7,394 inputs against CookieJar.split_header_words:
all strings of length 0 through 4 over a, equals, space, semicolon, comma, quote,
backslash, newline and é, followed by thirteen quoted/escaped/Unicode cases. The compact
serde_json token-vector digest is
`21a67ebcfe39a070ee0ea6a870d179e9fbae4ff4bcf31f4fe962122baa98a5c2`.
It runs without Python and was independently compared with the read-only source oracle.

The integer grammar corpus contains 6,928 inputs: short combinations of digits,
signs, underscores and invalid characters; every Unicode decimal digit and relevant
whitespace/control character in prefix/suffix positions; and 19/20/309/310/4300/4301-
digit values. Its compact serde_json result vector has the source-verified SHA-256
`fbebd0727da7c2bededb7ed78830a5d733431da1e106aacc71ba32c716ebf1d7`.
The separate Unicode table digest records each code point from 0 through 0x10FFFF
as its decimal value or byte 255, including surrogate code points as non-digits:
`bb3993b130cc02b9f2668f0460278cafc470fd87ebc8b5cccba421b579404a12`.
Both digests run without Python and were compared with read-only CPython processes.

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy python_integer -- --nocapture
```

```sh
HY_TEST_HTTPX_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy cookies -- --nocapture
```

Retention fixtures use temporary trees, explicit mtimes and outside symlink targets;
they prove strict cutoff handling, target preservation, empty-directory removal,
rejected-host/cancellation preservation and malformed/negative setting behavior.

Four Unix path tests compare seven paths with CPython when the optional oracle is
selected. They cover missing tails, repeated links, dangling internal/outside targets,
a cycle and a 64-link chain. The oracle only reads filesystem state; Rust creates the
temporary fixtures. To repeat against the verified runtime:

```sh
HY_TEST_PATH_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy ida::links::ke::paths
```

KE navigation fixtures advertise the exact temporary database path through an owned
IPC socket. One instance initially reports no loaded database, then reports it after
the fixture launcher publishes its marker; an open-only request must poll analysis
but send no navigation command. Other cases check literal forwarded JSON, repeated
and empty download parameters, missing launcher interpreters, startup timeout and
the distinction between launch failure dialogs and terminal-only navigation errors.
The open-only success message is asserted on stdout. No real IDA process is used.

Installer tests execute fixture installer programs under temporary homes. They
verify macOS installer application execution, product copying, dry-run, existing
destination preservation, failed/empty output rejection, optional default
registration, and authentication before license installation. EULA tests verify
the selected installation and supported registry API invocation with a fixture
interpreter; they do not execute IDA or accept a real EULA.

Instance tests use fixture SDK metadata and inert executables. They check manual
home/symlink resolution, byte-for-byte duplicate/cancellation preservation, invalid
arguments and paths, numeric and tie ordering, stale records, removal without
deleting installations, current-default prompt selection, native versus foreign
idalib libraries and independent deprecated configuration. A fixture `mdfind`
returns two temporary bundles; the test selects only those bundles, even when
system Applications contains other installations. Shared discovery tests cover
installer names, resolved duplicate order and first-Exec desktop parsing. The
desktop parser deliberately retains upstream's quoted-space limitation.

Compatibility integration tests exercise both supported CPU architectures using
fixture executable headers, platform override precedence, unknown-header failures,
service-pack range expansion, exact version membership and empty declarations.
The fixture headers are inspected, not executed. Version-oracle coverage consists
of 30 coercions, 35 specification-validity checks and 1,050 membership comparisons.

Bundle tests verify creation and installation through the CLI with fixture wheels,
all 30 target combinations, both repository override positions, per-platform
archive selection, pip options, offline transport, custom-source precedence,
missing targets, duplicate wheel filenames, hash failures, installation identity
checks and preservation
of existing output after failed downloads or source-distribution rejection. They
do not establish real wheel compatibility. Successful fixtures now declare the
required entry point, repository, contact and setting fields; malformed fixtures
are reserved for rejection tests. Model validation has a separate upstream oracle
and CLI boundary tests, including serialized defaults and extra metadata.

Update tests invoke a copied executable under a temporary home. They verify original
tags, prerelease selection, force/no-op behavior, default/SSH repository references,
pagination, token-bearing API requests, executable permissions, invalid assets and
preservation after empty, short, oversized and HTTP-error downloads. Replacement
payloads are inert fixture bytes and are never executed. The working executable
and installed developer tools are not replaced.

Root CLI fixtures cover unauthenticated and stored-interactive help, environment
API-key precedence, stale defaults, multiple unselected installations, missing and
active idalib, conflicting SDK/binary/directory versions, malformed configuration,
identity/version overrides and sorted command inventories. A local server records
zero requests during help; file bytes and absence of newly created configuration
directories prove help does not persist state. Tokens never appear in help output.
Fixture IDA binaries and libraries contain inert bytes and are never executed.

Download fixtures exercise both tag response forms, exact versus folded matches,
unresolved tags, direct pattern selection, leading-slash keys, mixed successful
and failed assets, missing URLs and terminal navigation. Cache fixtures distinguish
200/403/500 HEAD responses of identical length, force bypass, metadata-preserving
copies, empty overrides and rejected parent traversal. A bounded raw HTTP fixture
closes a declared 100-byte response after five bytes; the test checks the old cache
and target and the absence of leftover staging files. No remote assets are fetched.

License fixtures verify exact ID/plan/product predicates, distinct public IDs and
download keys, null-first stable ordering, signed downloads without account headers,
missing keys, empty URLs and continuation after failed assets. PTY tests cover
customer choice, initially unchecked licenses, custom installation paths and
creation decisions. Local copy tests verify bytes, timestamps, read-only source
permissions and rejection of direct/hard-link self-copy. The IDA installer fixture
fetches and publishes a license using only a synthetic account and inert file bytes;
failed acquisition leaves no license file and returns failure.

Share and asset fixtures cover all three visibility modes, case-normalized domains,
environment-key placeholders, metadata requirements and exact CSV payloads, hashes
across multiple read buffers, upload headers, missing/empty/null PUT URLs, malformed
tickets, transfer/confirmation errors, output naming and tilde expansion. Unix PTY
tests exercise visibility selection, overwrite/deletion cancellation and batch
continuation after an individual failure. Synthetic HTTP errors during shortcode
lookup, malformed descriptors and invalid local headers all propagate as failures.
All uploads, downloads and deletions target temporary fixture servers and homes.

The prior audit incorrectly classified ordinary shared-file HTTP errors as absent
records. AssetAPI catches HTTPStatusError, but APIClient._handle_response raises
custom APIError subclasses. A new source harness executes both classes together
through real HTTPX dispatch and an in-memory transport. It verifies six exception
outcomes, and twelve native get/delete scenarios now fail before any transfer or
deletion. The earlier isolated lookup mock did not establish the exception class
produced by the actual API client; that test and claim have been replaced.

The upload ticket oracle runs 75 cases: missing or malformed key/code/version
fields with and without a signed URL, eleven URL truthiness cases, and four
non-object responses. Upstream's actual upload methods execute against HTTPX
MockTransport, reading the same owned seven-byte file without writing it. Native
commands use a local server. The comparison records success and each HTTP method/
path in order, including PUT before invalid-key failure and confirmation before
invalid-code/version failure. The compact native result vector has SHA-256
`2be63a8dede6d49a8f97691a012493d1b7defd793b17fc8c678f08a73dec8f3d`,
pinned in the default suite after comparison with the locked source runtime.

Upload orchestration, generic shared selection and terminal-mode restoration now
occupy separate modules. The visibility selector starts at authenticated, displays
the user's domain, wraps with arrows/j/k/Ctrl-N/Ctrl-P, ignores Escape and exits
successfully on Ctrl-C/Ctrl-Q. A nonempty --code conflicts with force only after
visibility selection; cancellation happens first, and an empty code allows force.
Nine new PTY scenarios plus the two existing default/cancellation scenarios verify
these contracts. The same selector serves shared-file actions, including Ctrl-Q.
Upload result labels and generated URLs now print to stdout as upstream does.
Click's missing-path parsing stage, exhaustive display/markup equivalence and
native Windows terminal execution remain unverified.

```sh
HY_TEST_UPLOAD_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test asset_upload
cargo test --offline --locked --test share_operations --test share_selection
```

Bounded findings: **high impact**—moving ticket validation changes when bytes can
already have reached the server. The reference and native event traces now agree
for the stated corpus; no live uploads were used. **Medium impact**—truthy versus
false-valued URL fields affect whether transfer occurs, independently of final
model validation. **Medium impact**—prompt cancellation precedes the upload option
conflict check and must not initiate an API operation.

Authentication fixtures use a local HTTP server and synthetic keys/JWTs. They cover
upstream timestamp strings, credential insertion order, malformed stored data,
failed API-key validation and persistence, minimal/full refresh responses, account
mismatches, malformed sessions, invalid HTTP headers and credential precedence.
Failure injection replaces a fixture configuration destination with a directory
after it has been read; it does not depend on filesystem permission behavior.

API JSON decoding has a separate 104-case source comparison. Ten documents are
encoded in UTF-8, UTF-16LE/BE and UTF-32LE/BE, each with and without a BOM, followed
by four invalid byte sequences. They include literal non-BMP characters, escaped
Unicode, scalar/array/null values, 4,300/4,301-digit integers, an oversized integer
inside an unknown field, and a digit string unaffected by the numeric limit.
Accepted values and rejection decisions are compared with CPython json.loads.
The corpus does not establish unpaired-surrogate or non-finite-value equivalence.

Six CLI tests cover 53 scenarios: ten GET listings, twenty POST/DELETE responses,
ten encoded error messages, ten standalone identities, one oversized unknown
field and two malformed-error fallbacks. Every encoding fixture deliberately
declares a windows-1252 charset, verifying that JSON byte detection controls the
result. Unicode filenames, share codes, identities and error strings are retained.
Error response bytes are read completely before status classification; decoding
or integer-limit failures prevent an embedded error message from being selected.

The encoding detector now lives in util/json_encoding with distinct lossless
Unicode value decoding and surrogatepass syntax-validation entry points. The
number scanner lives in util/json_numbers; API use validates limits without
substituting values, while KE normalizes Python-only constants in a disposable
validation copy. All six KE metadata unit tests, including the existing 13,509-case
source corpus, pass after the extraction. Primary sources are CPython json.loads/
json.detect_encoding, HTTPX 0.28.1 Response.json, and the pinned APIClient JSON and
status-handling methods in src/hcli/lib/api/common.py.

```sh
HY_TEST_JSON_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy api_json_encodings
HY_TEST_JSON_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::links::metadata
cargo test --offline --locked --test api_json
```

Bounded findings: **high impact**—a syntax-only decoder may replace surrogate
code units without changing JSON validity, but cannot supply application values.
**Medium impact**—honoring an HTTP charset for JSON can corrupt an otherwise
valid Unicode response. **Medium impact**—an ignored model field still participates
in CPython's JSON integer-limit check and can invalidate the entire response.

Environment creation is now separate from dependency installation. The public
CreateOptions record names the requested path, explicit fallback version,
installation to probe, migration/configuration choices and output mode. The
installer supplies its newly installed directory directly, avoiding a process-wide
environment override or dependence on an earlier cached default-installation probe.

Three planning tests execute 38 read-only upstream cases: 24 explicit/probed version
combinations, eight registered/uv/PATH plans and six registered-interpreter layouts.
They exercise the actual functions in `src/hcli/lib/ida/python/venv_create.py` and
the candidate/name/shim helpers in `src/hcli/lib/venv.py` and
`src/hcli/lib/ida/python/__init__.py`. The source validator accepts Unicode decimal
digits and one final newline; it validates explicit input before probing, and a
successful probe remains authoritative when the two versions disagree.

Thirteen Unix CLI tests exercise 32 isolated processes. Fixtures cover missing and
empty targets, healthy reuse through python3, six unusable-target states with
existing file bytes preserved, registered uv arguments, continued PATH search,
ensurepip failure, migration success/package-failure/skipping, interpreter disappearance, malformed inline dependency
metadata, already-configured paths and two interactive confirmation sequences.
Synthetic uv/Python/idat programs are shell fixtures; no installed interpreter,
IDA installation, shell profile or system environment is changed. Creation and
dependency actions operate only inside the fixture directory.

Twelve of those CLI cases also compare the pinned `run_create_environment` and
`migrate_plugin_dependencies` command bodies. The oracle covers new versus healthy
environments, no/successful/failing dependencies and migration enabled/disabled.
All report fields are compared, except migration error text is reduced to a boolean
indicating its presence. Creation and pip are in-memory adapters in this source
oracle; the native counterparts execute the isolated fixture programs. This
proves orchestration/report behavior for the corpus, not real package installation.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy create::planning
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test create_environment
```

Bounded findings: **high impact**—letting an explicit version override IDA can
create an environment whose packages IDA does not load. **Medium impact**—reusing
a healthy environment must not trigger another dependency migration. **Medium
impact**—a pip package failure during migration does not invalidate successful environment
creation; upstream returns the failures in the report and exits successfully.
**Medium impact**—configuration can partially update files before failing; the
platform implementation now preserves upstream's distinction between command
failures and file exceptions, as detailed below.

Pip command construction and error rendering are separate, testable operations.
Options retain source ordering: isolated, disable-version-check, no-cache, offline,
index URL, extra indexes, find-links and no-build-isolation. Bundle flags and its
wheelhouse are merged before argv construction. The native backend no longer
injects a default version-check flag or a `--` delimiter, reconstructs the selected
interpreter's environment, or applies the former 600 s deadline. It explicitly
inherits stdin, while command output remains captured as bytes.

The source recorder compares 1,536 plans: two operations × 32 boolean option
combinations × three index values × two extra-index lists × two find-links lists ×
two bundle states. It executes the actual source pip helpers and asserts their
subprocess kwargs contain only capture_output=true and check=false, with no env,
stdin, cwd or timeout override. Native command inspection confirms no environment
or cwd override. The expected compact JSON vector has SHA-256
`3c6b7102c56f44e4965c5c29493f53a896cce60217c84d289cbba45f6c1094c5`.

A second corpus compares 162 failures: nine stdout byte sequences × nine stderr
sequences × two binary names. It covers empty streams, whitespace, malformed UTF-8,
Unicode, NUL, exact-case recognition and both known error signatures together.
The expected vector hashes to
`a33b36067c937af90e3e38019123079f96413e2c4084c9fcd505dd13a616c097`.
Both digests are pinned in the default tests after source comparison.

Two CLI tests run eleven processes. One supplies two stdin lines and explicit
PYTHONHOME, VIRTUAL_ENV, PYTHONUTF8 and PATH values; the two pip phases inherit those
values and consume one line each. Success output remains captured even when stderr
contains a known error signature. Ten failure cases exercise both phases and five
output shapes, verify failure-message routing, and prevent plugin publication.
Existing bundle and dependency fixtures retain argument-boundary, combined-set and
replacement-preservation checks. Migration now asserts the exact raw pip reason
and separately tests a fixture interpreter that removes itself after the first
plugin: the next OS launch failure aborts creation rather than becoming a package
failure report. The already-created target is retained.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::venv
cargo test --offline --locked --test python_pip
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test create_environment
```

Bounded findings: **medium impact**—pip inherits parent Python environment settings,
which can differ from the environment constructed for exec/script commands.
**Medium impact**—dependency text beginning with a dash remains option-like argv;
the corpus includes `--pre` and retains this source behavior. **Medium impact**—a
package-manager failure and an OS launch failure have different migration control
flow. **Low impact**—stdout precedes stderr in the combined reason regardless of
their temporal emission order; known signatures are examined only after failure.
**Low impact**—an explicitly empty index URL counts as a custom source while adding
no index argument, which can suppress automatic bundle-source selection.

### Plugin pip source paths and bundle downloads

The plugin callback now converts find-links before repository validation and
dispatch, including repository-free commands. Any string containing `://` is
passed through verbatim. Other values receive lexical pathlib spelling followed
by first-component home expansion. Empty input becomes `.`, repeated separators
and dot components are removed, and parent components remain. Relative home
values stay relative. POSIX retains exactly two leading slashes; Windows parsing
handles drive-relative, rooted, UNC and device paths without filesystem access.
An unknown user or unresolved tilde home produces the source message before a
repository-free command can run.

Production responsibilities are separated into find-links conversion, lexical
path spelling and host home lookup. Unix lookup uses reentrant libc account
functions and retains empty HOME as a set value, mapping it to `/` as CPython does.
Windows lookup observes USERPROFILE precedence, HOMEDRIVE/HOMEPATH fallback and
the source's named-user inference rule. Home paths with non-UTF-8 bytes are still
converted lossily on Unix; account-service error formatting and native Windows
execution remain outside the established A40 evidence.

The spelling corpus has 21,120 cases: two path flavors × 22 prefixes × twelve
components × eight suffixes × five home outcomes. Its compact JSON result vector
has SHA-256 `046cf8c48e7dd6aa6fefc6557762d26a2addc22ed6456201dd2bf28ade24f995`.
The 1,200 Windows home cases combine ten profiles, four current-user values, five
drive/path pairs and six requested users; their vector hashes to
`1240de0e17f4bfa48ca6c4ecb3884e35bdd0db15684f06d30f5823fb8ef9ec20`.
Six additional host account cases cover current/default users, missing users and
embedded NUL. The default suite retains both deterministic digests; account
database equivalence is checked when the read-only source oracle is enabled.

Bundle downloading now has its own small subprocess module. It appends target
arguments and destination, then index URL, extra indexes, find-links, offline
flag and dependencies. Installation-only isolated, cache, version-check and
build-isolation options are ignored, matching the source. Stdin, environment and
cwd are inherited; stdout/stderr are captured with no fixed deadline. Failure
decodes each stream as UTF-8 with replacement, preserves whitespace and inserts
one newline between stdout and stderr. Package-error classification remains a
separate installation policy.

The download corpus compares 2,304 plans: six platforms × two Python versions ×
32 flag combinations × three index values × two additional-source states. The
source target helper supplies its own platform tags. Its result vector hashes to
`2248c6de1a79faaf1df8e5bc66d56fe1b859837d45e906a8859858ccf9ae1eba`.
Thirty-six stdout/stderr byte pairs cover empty streams, whitespace, malformed
UTF-8, Unicode and NUL; their result vector hashes to
`627ce806f2b83ee9be01aae0d53b84b65c86b02707b71825c0d0d6d0b17b939a`.
The source recorder asserts subprocess kwargs contain only capture_output=true
and check=false. Native inspection verifies no environment or cwd override, and
CLI fixtures verify inherited stdin. No real pip operation runs in these tests.

Five added CLI tests cover eleven invocations. Installation verifies repeated
source order and normalization in both pip phases. Schema exercises conversion
before repository-free dispatch. Bundle creation verifies normalized sources,
argument order, inherited process state, captured success streams and raw
failure streams; existing output is retained after nonzero pip status. Empty and
relative HOME values also reach bundle pip with their source path semantics.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::pip
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy plugin::bundle::download
cargo test --offline --locked --test python_pip --test plugin_bundle_pip --test plugin_bundle
```

The oracle runtime contains CPython 3.13.15 and packaging 26.0, matching the
upstream lockfile. The pre-existing extension runtime has packaging 26.3; its
Intel macOS tags use `fat3` where the locked version uses `fat32`. The native
implementation retains the locked spelling. The download oracle checks the
dependency version explicitly instead of attributing this runtime difference to
the Rust port.

Bounded findings: **medium impact**—dependency-version changes can alter the wheel
candidate set even when the HCLI source revision is unchanged. **Medium impact**—
unknown-home conversion can fail a command that never accesses a plugin repository.
**Medium impact**—sharing installation option construction with downloading changes
source order and introduces flags the download helper ignores. **Low impact**—the
URL distinction is a literal substring rule, so unusual strings containing `://`
retain their original spelling. These results depend on A40 and A41.

### Bundle target grammar and selection

Target validation now uses a small grammar module, while CLI selection controls
expansion and observation order. Python versions follow the source decimal regex:
Unicode decimal digits are accepted, signs and underscores are rejected, and one
final LF is allowed by the source `$` anchor. The supplied version spelling is
retained after numeric validation. Numeric comparison uses arbitrary-precision
integers with CPython's default 4,300-digit conversion limit; oversized values
produce its conversion-limit diagnostic. Explicit target IDs split their CPython
major digit by Unicode character rather than UTF-8 byte offset. Malformed versions,
platforms and target IDs use Python string repr in errors, including quotes and
control characters; invalid target IDs retain the source usage hint.

Ordinary `--platform` options resolve aliases before building the Cartesian product.
Explicit `--target` IDs and `--platform current` observations validate the platform
but preserve its supplied spelling. A preserved alias such as `windows` therefore
passes selection and fails the source's later canonical tag lookup. Tag generation
now returns an error instead of inventing a platform tag from an unknown string;
download and manifest construction propagate that error. Canonical platform tag
sequences remain covered by the packaging 26.0 download corpus under A41.

Missing platform and Python options have separate ordered diagnostics. Mixing an
explicit target with either option fails before observation. Platform expansion
precedes Python expansion; repeated `current` entries perform repeated observations.
The expanded product is deduplicated by raw target ID while preserving first-seen
order. Explicit target lists are returned without deduplication, as in the source.
The source can consequently create a duplicate-ID manifest that its own reader
rejects; the native CLI preserves that distinction.

Current Python detection now resolves IDA's interpreter and invokes the independent
version-only helper instead of the broader environment probe. It reports the source
failure message when the helper returns no version. CLI fixtures confirm repeated
version probes, preserved PYTHONHOME/VIRTUAL_ENV/PYTHONUTF8 values, stripped version
output and distinct handling of empty output, nonzero exit, invalid syntax and a
below-minimum version. The shared-probe differences identified in that pass are
addressed under A43 below: stdin is inherited, both streams are decoded before
status inspection, universal newlines are translated, and malformed text follows
caller-specific error boundaries. Native Windows and non-UTF-8 locale handling
remain open.

The grammar corpus compares 576 cases: ten platform spellings each receive an alias
check and 28 version/explicit-ID pairs, followed by six additional malformed IDs.
Versions include Unicode digits, LF/CRLF, signs, underscores, quotes, NUL, values
above u32 and components of 4,299, 4,300 and 4,301 digits. Successful targets include
their preserved fields, ABI list and tag result; failures compare exact source
messages. Its compact result-vector SHA-256 is
`1d6807459362b94b4628c42dbc7e21f03288c6d4768601895f34f6adbf49d20c`.

The selection corpus compares 1,800 cases: nine platform lists × ten Python lists ×
four explicit-target lists × five runtime outcomes. It compares the entire ordered
target list or error together with the ordered platform/Python observation calls.
Its result-vector SHA-256 is
`438838f8755cb0c91d55a3871d35c083552c486cd7a5b3168884ea9ec9c1f6a2`.
Both digests are retained in the default suite after executing the actual source
functions. Five additional CLI tests cover sixteen invocations, including manifest
contents and preservation of existing output after late tag-generation errors.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy plugin::bundle::targets
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy cmd::plugin_bundle_targets
cargo test --offline --locked --test plugin_bundle_targets --test plugin_bundle --test plugin_bundle_pip
```

Bounded findings: **medium impact**—normalizing a hidden explicit target alias changes
failure into success and alters archive identity; validation alone must not rewrite
its spelling. **Medium impact**—deduplicating explicit target lists would conceal a
source writer/reader inconsistency. **Medium impact**—reusing a broad interpreter
probe changes command arguments, environment and accepted output for a version-only
observation. **Low impact**—a final LF and non-ASCII decimal spelling survive into
version and target fields despite numeric validation. These results depend on A42.

### Reference path grammar and directory existence

Under A61, `src/plugin/files.rs` implements the source's POSIX reference grammar
independently of extraction policy: ASCII strings are accepted unless they begin
with `/` or contain a slash-delimited `..` component. Empty strings, colons, literal
backslashes and ASCII control characters pass this grammar. Acceptance by the
grammar does not establish that the referenced object exists or can be opened.
The source functions are `validate_path` and
`does_plugin_path_exist_in_plugin_archive` in the pinned
`src/hcli/lib/ida/plugin/__init__.py`.

Directory validation follows `validate_metadata_in_plugin_directory` in the pinned
`src/hcli/lib/ida/plugin/install.py`: the bare entry is checked before native suffix
alternatives, and existence includes directory objects and followed symlinks.
Missing paths, non-directory parents, link loops and embedded NUL paths return
false; other stat failures propagate. `src/util/python_path/exists.rs` implements
this CPython 3.13 policy. Its Windows errno mapping follows
[CPython 3.13.15 PC/errmap.h](https://raw.githubusercontent.com/python/cpython/v3.13.15/PC/errmap.h)
and the runtime's separate ignored `winerror` values. Native Windows execution
has not been verified. Regular directory packaging still validates files rather
than directory records, since the source ZIP writer omits those records.

Filesystem references and inline dependency reads use the shared native lexical
join. `src/util/python_path/path/join.rs` joins raw path spellings before component
normalization. This order matters for incomplete UNC prefixes: joining `//` and
`.` on Windows produces `\\\\.`, while joining already-normalized components loses
the dot. Drive-relative joins preserve the base on the same drive and replace it
on a different drive; rooted paths preserve an existing drive when appropriate.
The same join is used for archive reference names. `safe_join` was removed after
its remaining production consumer, inline dependencies, adopted the source's
native path lookup. Selected archive extraction retains its own validation pass.
The CPython adaptation notice and PSF license are in `src/util/python_path/LICENSE`.

Evidence:

- 3,329 strings compare the actual source validator: every string of length zero
  through four over seven path-relevant characters, four placements of every ASCII
  character, and sixteen explicit drive/UNC/Unicode cases. Each is also joined to
  thirteen bases under both pure path classes: 86,554 joins. Digest:
  `7cf47470e992622349fff314a2e1f61e15ff9466fe1915e7b8f9c7d30c6be89e`.
- 196 real directory fixtures cover fourteen reference spellings in both entry
  and logo fields, with seven missing/file/directory/link states. They compare
  the actual directory validator and inline-dependency reader independently.
  Digest: `2acc0c7328e1f1b283a0e6bab9b5ac0ca9e24cf91679df1ce74e0ea2b7b98ee1`.
- A permission-denial probe verified that both native and source existence checks
  propagate the error. The test reports when its process privileges make the
  probe ineffective; that bypass did not occur in the recorded macOS run.
- CLI regressions exercise lint, indexing, installation and fake-pip invocation
  for colons, literal backslashes and normalized separators. A separate regression
  checks directory/empty references through lint and editable registration.

Rust creates every fixture; the Python oracle uses `-I -B` and rejects filesystem
mutation through its audit hook. Source read failures are compared separately from
path-grammar acceptance. These cases do not establish full metadata-model behavior,
exact error text, native Windows filesystem semantics, arbitrary stat failures,
non-UTF-8 path equivalence or source mutation during inspection.

The configured-environment extension decision was also verified against the
existing bridge: all nine `tests/extensions.rs` tests passed with
`HY_TEST_EXTENSION_PYTHON=/tmp/hy-extension-runtime/bin/python` and
`--include-ignored --test-threads=1`. The user-facing setting remains
`HCLI_EXTENSION_PYTHON`, documented in `docs/extensions.md`. These tests include
seven runtime-dependent cases plus two configuration cases.

The uninterrupted serial all-target run passed 655 tests (247 unit, 408 integration)
across 62 suites, with nine existing opt-in tests ignored and no failures. This run
enabled the two established source runtimes and the lint oracle. Rustfmt, whitespace
validation, Clippy with warnings denied and the Windows all-target cross-check
passed. The initial Clippy run required changing the permission fixture's zero
mode literal to octal notation; its value and behavior are unchanged. Upstream
remained clean at the pinned revision. The full-suite command was:

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
HY_TEST_LINT_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
cargo test --offline --locked --all-targets --no-fail-fast -- --test-threads=1
```

**High impact:** a Windows-oriented generic path filter rejected valid literal
Unix filenames and prevented inline dependency reads. **Medium impact:** regular-file
checks excluded source-valid directory references from lint and managed inventory.
**Medium impact:** swallowing stat errors obscured terminal filesystem failures.
**Low impact:** normalizing before joining changed incomplete UNC path spellings.

For B path bytes, validation and lexical joining take O(B) work; joining retains
O(B) text. Existence adds one filesystem metadata lookup per candidate, with at most
three native suffix alternatives. Filesystem latency and link resolution are not
bounded by these CPU/storage counts. Inline parsing retains the script bytes as
before; arbitrary script size and the full PEP 723/TOML grammar remain separate limits.

The A61 review identified regular directory packaging as the next integration gap.
The former implementation read the root descriptor and copied recursively with
`std::fs::copy`, preserving file permissions. A62 replaces that path as described
below. The earlier recursive traversal and source's global component sort were
different algorithms; no final file-order discrepancy was established for stable,
ordinary Unix trees solely from that distinction.

### Regular directory packaging and retained installation sources

Under A62, `src/plugin/install/source.rs` separates editable directory references
from owned archive snapshots. Regular directory inputs are packed once by
`src/plugin/install/directory.rs`; file inputs are read once into the same archive
representation. Descriptor inspection and staging consume that retained reader.
The former directory-copy implementation and its separate distribution validator
were removed. Editable sources remain live directory references.

The source contracts are `pack_plugin_directory_to_zip` in the pinned
`src/hcli/lib/ida/plugin/install.py` and the acquisition branches in
`src/hcli/commands/plugin/install.py`. Packing gathers the complete tree before
writing members in pathlib component order. Directory links are not traversed;
file links are dereferenced. Directory-type checks precede exclusion filtering.
Unreadable directory scans are suppressed, while an included unreadable file,
dangling link, link cycle or socket can terminate packing. Empty directories are
omitted. Files beneath `.git`, `.hg`, `.svn`, `__pycache__` or `.DS_Store` are excluded.

This ordering moves packing failures ahead of metadata/version checks. A second
valid descriptor fails direct installation even if it has the same plugin name
or the installed version is already current. Conversely, invalid root metadata
can coexist with one valid nested descriptor: the archive selector installs that
descriptor's subtree. Excluded descriptors do not count. Archive extraction creates
new files without restoring source execute permissions. Home expansion and directory
aliases resolve before acquisition; a bare directory without a root descriptor falls
through to repository selection.

Evidence:

- Ten packing fixtures compare the actual upstream function with native output.
  The compound tree covers component ordering, Unicode, literal colon/backslash
  names, included development directories, nested exclusions, executable files,
  file/directory links and an excluded dangling link. Five timestamp cases cover
  ordinary, odd-second, fractional-rounding, pre-1980 and post-2107 dates. Separate
  cases cover included dangling/loop links, sockets and an empty tree.
- Two effective permission-denial comparisons distinguish a suppressed directory
  scan from a terminal file-open error. The test reports a privilege bypass if the
  process can still read its mode-zero fixtures.
- Seven CLI regressions cover descriptor multiplicity, nested selection, exclusions,
  failure before an equal-version upgrade skip, installed/source modes, home/link
  resolution in regular and editable modes, and bare-directory repository fallback.
- A unit test deletes the original tree or replaces the original ZIP after acquisition,
  then verifies the retained descriptor and extracted payload for both input kinds.
  It does not model in-place mutation during the initial read or full publication.

Rust materializes all fixture files. Python runs with `-I -B` and an audit hook that
rejects filesystem mutation; the upstream ZIP writer only writes `BytesIO`.
Successful comparisons inspect ordered member names, decoded SHA-256 values, sizes,
DOS timestamps, compression method and rwx permission bits. Errors compare terminal
failure, not exception text. These tests do not claim identical compressed ZIP bytes.
The native ZIP writer discards setuid/setgid/sticky bits from external attributes;
installation does not restore those attributes. Complete archive metadata identity,
native Windows creation attributes and case-tie traversal remain outside this result.

**High impact:** inspecting only root metadata accepted directory trees that upstream
rejects as multiple-plugin distributions. **Medium impact:** deferring file reads
until staging concealed packaging errors behind upgrade skips. **Medium impact:**
reopening an archive between inspection and extraction allowed its content to change;
retained acquisition now removes that interval for regular sources. **Low impact:**
copying source modes made installed executability differ from archive installation.

For N entries and P aggregate path bytes, collection and cached sort keys retain
O(N + P) storage. Sorting takes O(N log N) comparisons plus component-byte comparison
costs. Packing reads B source bytes and retains C compressed bytes plus ZIP member
metadata and codec buffers; compression cost depends on the codec. File inputs retain
their entire archive in memory, matching source acquisition. The archive remains
resident through staging. No size quota, whole-operation deadline, or atomic snapshot
of a concurrently modified source tree is introduced.

The first A62 full run passed 664 tests and failed one stale diagnostic assertion
in `tests/plugin_manifest.rs`. It expected regular directories to validate their
root descriptor directly. That test now expects archive discovery for regular
directories and adds editable-source cases to retain coverage of direct field
diagnostics. Across four invalid descriptors and four source kinds, all sixteen
cases still assert failure before dependency invocation or publication.

The final uninterrupted serial all-target run passed 665 tests: 250 unit tests and
415 integration tests across 62 suites, with nine existing opt-in tests ignored and
no failures. It used the full-suite command recorded under A61, including both
source runtimes and the lint oracle. All nine extension tests passed separately
with `HY_TEST_EXTENSION_PYTHON=/tmp/hy-extension-runtime/bin/python` and
`--include-ignored --test-threads=1`. Rustfmt, whitespace checks, Clippy with warnings
denied and the Windows all-target cross-check passed after the fixture correction.
The upstream checkout remained clean at the pinned revision. QG3 and QG5 remain
open for the outstanding contracts and edge cases recorded in this audit.

The A62 review identified acquisition differences separate from packing: local
archive suffix recognition and GitHub branch selection. A63 addresses those
contracts below; the packing comparisons alone did not cover them.

### Direct installation acquisition and GitHub release spelling

Under A63, `src/cmd/plugin_ops/source.rs` owns source classification in the order
used by `install_plugin` in the pinned `src/hcli/commands/plugin/install.py`.
Editable inputs require a directory and retain direct descriptor validation.
Regular directory recognition expands the home prefix and requires a root
`ida-plugin.json` file. Local archive recognition instead checks the original
path spelling and its exact lowercase `.zip` suffix, without requiring a regular
file. A directory named `source.zip` without root metadata reaches a file-read
failure; a root descriptor makes it a directory distribution. Existing files
without that suffix fall through to repository selection.

The remaining branches are exact `file://` prefix, the shared source-compatible
GitHub predicate, exact `https://` prefix, and repository reference. Direct `http://`
and short `file:` inputs no longer enter download transport. This does not prevent
repository locations or GitHub release assets from using those transport schemes
where their source contracts allow them.

Acquisition now passes `InstallationSource` into the shared installation operation.
Explicit directory and archive constructors prevent a later filesystem-type check
from changing the selected branch. Bundle-member installation and repository
upgrades pass their fetched bytes directly, removing temporary write/read cycles.
Editable registration obtains its source path from the retained editable variant.

`src/plugin/index/transport/github_url.rs` follows `parse_github_url` from the pinned
`src/hcli/lib/ida/plugin/repo/github.py` over inputs accepted by
`is_github_direct_install_url`. It uses the existing shared predicate from A51
instead of another URL recognition rule. Recognition is case-insensitive with
CPython's represented Unicode exceptions. Parsing still requires an ASCII HTTPS
scheme and the actual GitHub authority; Unicode characters that merely match the
regex do not automatically become valid authority characters.

The parser splits a raw tag before URL parsing and strips trailing `/` from that
tag. A terminal LF can therefore survive in the tag. For the URL portion, the
accepted terminal LF is removed as by `urlparse`. Repository `.git` removal occurs
once and before empty path components are discarded: `repo.git/` retains `.git`,
whereas `repo.git.git` becomes `repo.git`. Dot segments and Unicode path components
are preserved at this parsing stage. Downstream HTTP URL serialization is a
separate contract and is not proved by these tuple comparisons.

Evidence:

- 1,822 acquisition comparisons cover 24 physical path fixtures, 880 scheme/host/
  path/suffix combinations and seven additional inputs, each in regular and editable
  mode. The Python oracle reads the actual command AST, retains its branch conditions
  and editable path checks, and replaces acquisition bodies with branch labels.
  It does not invoke the source downloader, packer, pip or publication path.
- 25,088 comparisons invoke the actual source GitHub predicate and, for recognized
  inputs, its parser: four schemes, four authorities, four owner spellings, eight
  repository spellings, seven tag forms and seven suffixes. They compare recognition,
  parse failure, owner, repository and raw tag, including case, Unicode, `.git`,
  dot components, terminal LF and rejected query/fragment spellings.
- Four CLI regressions verify repository fallback despite an existing non-ZIP file,
  uppercase suffix rejection, `.zip` directory precedence, no transport for rejected
  direct schemes, installation through a file URL with a non-ZIP suffix, and four
  exact GitHub release request paths through an owned server. Existing directory,
  editable, upgrade and MCP consumers also pass.

Rust creates every fixture. Both Python oracles run with `-I -B` and audit hooks
that reject filesystem mutation. Error projections compare failure status rather
than full Click/Pydantic exception text. The source runtime, filesystem assumptions
and unverified native Windows execution are bounded by A45/A63.

**High impact:** an existing arbitrary file could shadow the repository plugin the
source would select. **Medium impact:** prefix-only GitHub detection selected a
different acquisition operation for uppercase hosts and extra URL path segments.
**Medium impact:** normalizing the repository path before extracting owner/repository
changed release endpoints. These changes affect selection, not only presentation.

For L input bytes, recognition and parsing take O(L) work and retain O(L) path/tag
text. Classification performs a bounded number of filesystem probes; path resolution
and filesystem latency have no added deadline. Those bounds exclude downloading,
repository selection, archive parsing and installation. Source mutation between
classification and acquisition remains unverified.

The uninterrupted A63 serial all-target run passed 671 tests: 252 unit tests and
419 integration tests across 62 suites, with nine existing opt-in tests ignored and
no failures. It used the full command recorded under A61 with both established
source runtimes and the lint oracle enabled. The new oracle corpus sizes are asserted
in their tests. Rustfmt, whitespace validation, Clippy with warnings denied and the
Windows all-target cross-check passed. The source checkout remained clean at the
pinned revision. The preceding A62 extension suite remains the latest separate
all-nine-extension validation. Full parity is not established: QG3 and QG5 remain open.

The A63 review identified archive transport as the next integration gap. A64
addresses file-URL decoding below. Repository HTTP fetching still differs in
redirect-status selection, cached credential resolution and entitlement-error
mapping. GitHub release fetching has its own request headers, timeout values and
release-response defaults. A63 proves source selection and represented release
request paths, not those transport contracts.

### File-URL paths for archives and repositories

Under A64, `src/plugin/index/transport/file_url.rs` converts local archive URLs
before a WHATWG URL parser can normalize their paths. Archive fetching and
repository construction use the same adapter. The pinned source contracts are
`fetch_plugin_archive` and `repo_from_url` in
`src/hcli/lib/ida/plugin/repo/__init__.py`, plus `JSONFilePluginRepo.from_url` in
`repo/file.py`. Each converts the parsed URL's path through `url2pathname` and
then constructs a native `Path`.

The adapter strips source-defined URL controls, recognizes the file scheme without
case sensitivity, validates a present authority, and separates path from query and
fragment. It does not use the authority as a filesystem host. Semicolons remain in
file paths. The `url2pathname` handling of an empty authority, extra leading slashes
and a literal lowercase `localhost` prefix occurs before percent decoding. Native
path normalization removes redundant separators and `.` components while retaining
`..`. Consequently, `link/../plugin.zip` is resolved by filesystem traversal; erasing
that parent segment through URL normalization can select a different archive.

On Unix, percent-decoded bytes represent UTF-8 plus surrogateescape directly in an
`OsString`, retaining bytes that are not valid UTF-8. On Windows, drive/bar conversion
and UTF-8 replacement decoding precede the existing PureWindowsPath adapter. Encoded
slashes, encoded colons, malformed percent escapes and drive errors follow these
separate conversion orders. Native Windows filesystem execution is not established
by comparing its conversion through pure path objects.

Authority validation remains necessary even when the authority is discarded for
file selection. `url_parts/authority.rs` checks bracket structure, IPv6 scope spelling
and represented IPvFuture forms. Its NFKC delimiter classification is the nineteen
non-ASCII Unicode 15.1 characters whose normalization introduces `/`, `?`, `#`, `@`
or `:`. This avoids inheriting another library's Unicode table version. Attribution
and adaptation details are recorded in `src/util/python_path/LICENSE` for the
CPython `urllib.parse`, `urllib.request` and `nturl2path` code.

Evidence:

- 5,315 source conversion comparisons comprise 2,660 scheme/authority/path cases,
  2,560 percent-octet cases and 95 delimiter/combining-context cases. They compare
  parsing failure, POSIX filesystem bytes and Windows normalized path strings.
  The source uses actual `urlparse`, `url2pathname`, `nturl2path.url2pathname` and
  pure path classes. Separate enumeration over the non-ASCII Unicode range verifies
  the complete nineteen-character NFKC delimiter table against `unicodedata`.
- Forty-five calls compare actual `fetch_plugin_archive` reads with native fetching
  across five authority forms and nine path spellings. Fixtures cover trailing
  slash/dot, semicolon and space names, percent-decoded invalid bytes, missing/NUL
  paths and symlink/parent traversal selecting a different payload.
- A CLI regression installs version 2 through `file://elsewhere/.../link/../plugin.zip`
  while version 1 occupies the path produced by premature dot-segment removal.
- Six repository CLI/source comparisons construct directory, bundle and JSON
  repositories through nonlocal authorities, uppercase file schemes and the same
  symlink/parent path. They compare complete snapshots from actual `repo_from_url`.

The initial physical-file fixture failed because this macOS filesystem returns
EILSEQ when creating a filename containing byte FF. The corrected fixture reports
that constraint and compares native/source read failure for that name; it does not
skip the remaining read cases. The pure conversion tests separately verify retained
invalid UTF-8 bytes. Successful physical reads of such names remain unverified on
this volume. The source oracles use `-I -B` and audit hooks rejecting filesystem
mutation; all fixture creation and modification is performed by Rust.

**High impact:** WHATWG dot-segment removal could install the wrong local archive
when a path traverses a directory symlink before `..`. **Medium impact:** authority
restrictions rejected file URLs that upstream treats as local paths. **Medium
impact:** percent decoding and pathname conversion have different orders and error
policies on Unix and Windows; sharing a generic URL-to-file conversion concealed
those distinctions.

For L URL bytes, decoding, authority checks and lexical path normalization take
O(L) work and O(L) retained storage. File reads retain B bytes, and subsequent archive
processing has its existing resource costs. The adapter adds no filesystem deadline
or atomic snapshot guarantee. Full exception text, arbitrary authority combinations,
other Unicode/filesystem encodings and native Windows execution remain outside the
verified result. The remaining HTTP and GitHub transport contracts are still open.

The uninterrupted A64 serial all-target run passed 676 tests: 255 unit tests and
421 integration tests across 62 suites, with nine existing opt-in tests ignored and
no failures. It used the full command recorded under A61, including both established
source runtimes and the lint oracle. The file-URL corpus and physical read counts
are asserted in their tests. Rustfmt, whitespace checks, Clippy with warnings denied
and the Windows all-target cross-check passed. The upstream checkout remained clean
at the pinned revision. These results do not close QG3 or QG5 for the remaining
transport, platform and metadata contracts.

### Repository HTTP redirect, credential and response policy

Under A65, `src/plugin/index/transport/repository.rs` implements the request loop
from `fetch_plugin_repo_bytes` in the pinned
`src/hcli/lib/ida/plugin/repo/__init__.py`. The source was read directly from the
local checkout at the revision recorded above. Its `is_plugin_repo_host` helper
uses raw `urllib.parse` hostname rules before HTTPX normalizes the request URL.
The shared `url_parts` adapter now supplies those pieces to both repository host
classification and A64's file-URL conversion. In particular, a percent-encoded
hostname does not become credential-eligible merely because a WHATWG parser would
decode it to the repository host.

Credential resolution occurs on the first eligible hop and is cached for that
fetch, including an empty result. Eligible destinations are HTTPS on
`plugins.hex-rays.com` or its subdomains. Requests to mirrors omit those headers.
The generic repository fetch no longer injects `GITHUB_TOKEN` into GitHub requests;
the source repository helper does not do so. The independent GitHub GraphQL client
retains its explicitly configured token behavior.

Only 301, 302, 303, 307 and 308 responses with a Location header are followed.
An empty Location is still a redirect; its represented join preserves the base
fragment. The loop permits eleven requests, counting the initial request and ten
followed redirects. On the final response, redirect target construction and the
original-HTTPS downgrade check precede exhaustion reporting, as in the source.
A fetch that began with HTTP is not subject to that original-HTTPS check after an
intermediate upgrade. Cookies use the existing HTTPX-compatible jar for one fetch
and are not shared with the next fetch.

The production client consumes and decodes response bodies before redirect and
status decisions. Invalid compressed redirect or error bodies therefore fail at
decoding rather than following a Location or presenting a status error. The native
client disables automatic reqwest decompression and uses the existing shared
gzip/deflate decoder. Its request advertises those two codecs.

`PluginAccessDenied` preserves the initial URL, status, authentication state and
optional repository name. Its display follows the source exception: absent
credentials prompt login; authenticated 401 distinguishes rejected credentials;
authenticated 403 identifies an entitlement denial. `load_named` attaches repository
context; generic archive fetching remains unnamed. Other failures retain HTTP
status and URL instead of translating every 401/403/404 into API authentication
messages. Generic HTTP error text is native and does not reproduce HTTPX's complete
`HTTPStatusError` presentation.

Evidence:

- 231 comparisons invoke the actual upstream fetch loop with HTTPX MockTransport.
  They compare request URLs, selected request headers, credential-resolution counts,
  returned bytes, error categories and all structured entitlement fields plus
  diagnostic text. Cases span seventeen statuses, missing/empty/nonempty Location,
  credentialed and mirror hosts, cached empty credentials, cross-host chains,
  redirect counts around ten, original-scheme downgrade policy, fragments,
  Latin-1/UTF-8 Location headers, cookie return and corrupted gzip responses.
- 280 comparisons invoke the actual source host helper across five schemes,
  fourteen authority forms and four suffixes. Percent encoding, userinfo, invalid
  ports, bracket forms, Unicode delimiters, suffix spoofing and control cleanup
  are represented. Invalid authority outcomes are compared as failures, not by
  full exception text.
- Three CLI regressions run 37 owned-server scenarios: thirty status/Location
  combinations, six gzip/status combinations and one three-hop cookie session.
  They exercise actual reqwest requests and complete repository snapshot output.
  Loopback requests omit configured API and GitHub credentials. These HTTP fixtures
  do not certify TLS credential delivery to actual repository hosts.

The oracle runs with `-I -B` and a filesystem-mutation audit hook. Its initial run
caught an upstream import attempting configuration migration; the audit rejected
the write. The corrected fixture redirects source configuration discovery to a
Rust-created configuration whose version already matches the pinned upstream.
The only patched runtime boundaries are client construction, optional credential
resolution and configuration-directory discovery. Python performs no file edits.
The comparison found and corrected empty-Location fragment loss. All previous
file-URL and direct-GitHub parsing comparisons passed after the shared URL-parts
refactor.

**High impact:** normalizing raw hosts before credential eligibility can change
which requests carry credentials. **Medium impact:** resolving credentials at each
eligible redirect repeats authentication work and can change state during a fetch.
**Medium impact:** ignoring response decoding or cookies changes acquisition even
when status and Location appear correct.

These findings are bounded to the represented policy and wire cases. The native
redirect join still uses `url::Url` except for the tested empty-Location adjustment;
arbitrary HTTPX URL serialization and joining are not established. The existing
native 30 s total request deadline differs from HTTPX's per-phase timeout contract.
Proxy discovery, TLS trust, optional codecs, malformed framing, complete generic
error text and native Windows execution remain open. The mock compares whether a
User-Agent exists, not identity equality: Hy retains its own product/version.
Named-error formatting is compared by supplying the name to the structured error;
the oracle does not exercise native `load_named` against an eligible TLS server.

For R requests (R ≤ 11), U aggregate URL/header bytes and B aggregate decoded body
bytes, parsing and decoding take O(U + B) work, excluding codec-specific cost and
cookie matching. For C stored cookies, each outgoing selection has O(C log C)
sorting cost plus cookie/path byte comparisons. Retained storage is O(U + K + M)
for K stored cookie bytes and the current maximum response M bytes, plus decoder
state. No new byte quota or whole-fetch deadline is introduced. Authentication may
perform separate work; the request limit does not bound its duration.

The uninterrupted A65 serial all-target run passed 681 tests: 257 unit tests and
424 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. It enabled both established source runtimes and the lint oracle,
using the full command recorded under A61. The repository corpus sizes are asserted
in their tests. Rustfmt, whitespace validation, Clippy with warnings denied and
the Windows all-target cross-check passed. The source checkout remained clean
at the pinned revision.
These results do not close QG3 or QG5 for the remaining contracts.

### Direct GitHub release selection and HTTP acquisition

Under A66, `transport/github_release.rs` owns selection and diagnostics, while
`transport/github_http.rs` owns the two HTTP operations. The source contract is
`fetch_github_release_zip_asset` in the pinned
`src/hcli/lib/ida/plugin/repo/github.py`. `transport/response.rs` now shares complete
body decoding with A65's repository transport; their redirect and status policies
remain explicit at each caller.

Release metadata uses `Accept: application/vnd.github.v3+json`; the asset uses
`Accept: */*`. Each operation creates its own client and cookie jar. Cookies persist
over that operation's redirects but metadata cookies do not authorize the later
asset GET, even when both URLs have the same host. Neither operation resolves HCLI
credentials or injects `GITHUB_TOKEN`. Initial URL userinfo becomes Basic auth;
represented same-origin, cross-origin and default-port HTTP-to-HTTPS transitions
follow HTTPX's header retention rules. Redirect URL userinfo does not independently
replace the existing Authorization header through reqwest.

The native GET loop follows 301, 302, 303, 307 and 308 with Location and permits
twenty redirects (at most twenty-one requests). It constructs the redirect target
before consuming/decoding that response, matching HTTPX's automatic redirect
order. A missing or empty target fragment inherits the previous fragment in the
represented cases. The final decoded response is checked for HTTP failure before
checking whether an originally HTTPS operation ended on another scheme. Unlike
the repository helper, temporary downgrades followed by an HTTPS final response
are permitted by this source function. Native connect/read deadlines are 30 s for
metadata and 60 s for the asset, without a total-operation timeout. HTTPX's write
and pool deadline contracts are not established by these settings.

Selection no longer deserializes every asset into a rigid struct. Missing `assets`
defaults to an empty collection; empty string/object collections have the source's
empty-iteration outcome. Missing names default to an empty string. ZIP names are
selected case-insensitively, and all names are examined before candidate count is
resolved. Non-ZIP entries do not require a download URL or a valid size. The sole
candidate defaults a missing size to zero and reads its download field before
comparing size. Negative sizes, fractional values and booleans follow the source
comparison; the threshold is exactly 104,857,600 bytes (100 MiB). Only metadata size
is checked—no new actual-body quota is imposed. No-asset, multiple-asset and
oversize diagnostics match the source strings, including tag/owner/repository and
Python numeric formatting.

The A66 implementation used the existing CPython JSON encoding adapter and
4,300-digit integer limit before serde decoding. That accepted the represented
UTF-8 BOM, UTF-16 and UTF-32 documents, but rejected literal NaN/Infinity tokens
and unpaired surrogate strings and imposed a different nesting limit. A67 below
replaces that release decoder. Finite JSON numeric syntax that overflows binary64,
such as `1e309`, was covered separately from those literal tokens under A66.
A separate read-only probe confirmed that source sizes `NaN` and `-Infinity`
select the archive, while `Infinity` produces the policy error with `inf bytes`.
The A66 native serde decoder rejected these three literal forms before selection;
those observed source outcomes were not counted as passing A66 comparisons.
They are covered by the expanded A67 comparisons.

Evidence:

- 1,998 selection comparisons comprise 666 documents under absent, empty and
  nonempty tags. They invoke the actual upstream function, intercept its GET
  boundaries and compare selected URLs or failure categories. Policy errors also
  compare exact messages. The corpus includes twelve name forms, thirteen size
  forms, four download-field forms, irrelevant assets, malformed containers,
  ambiguous candidates, integer limits, binary64 rounding/overflow and five byte
  encoding variants. The oracle asserts both source GET keyword sets, including
  timeout values, Accept and redirect enablement. It does not download those URLs.
- 125 response-sequence comparisons invoke the actual upstream GitHub function
  with HTTPX client redirects over MockTransport. The second GET returns a fixed
  archive after successful metadata acquisition. Comparisons cover the first
  operation's request URLs, Accept/encoding/auth/cookie headers and result category:
  seventeen statuses, Location presence, limits around twenty, final downgrade
  and HTTP-error precedence, temporary downgrades, Basic auth retention/removal,
  fragments, Latin-1/UTF-8 headers, cookies and corrupt gzip bodies. Success is a
  category projection; these mock cases do not compare archive contents.
- Three additional CLI regressions cover eleven owned-server scenarios: three
  successful compressed acquisitions with size coercion and UTF metadata, two
  redirect-limit cases, three selection failures and three compressed-response
  failures. Successful cases verify installed files, request Accept values and
  omission of configured API/GitHub credentials. The twenty-redirect case verifies
  cookie return within metadata, absence at the asset operation's first request,
  and a fresh asset cookie returned on its redirect. Existing A63 endpoint-spelling
  CLI cases also pass.

Both new source oracles execute with `-I -B` and filesystem-mutation audit guards.
Rust owns all fixtures. Mock transport comparisons are not evidence of live TLS,
proxy selection, timeout timing or native Windows networking. Generic HTTP and
malformed-data exception text remains native; only the specified policy messages
are compared exactly. User-Agent retains Hy's identity rather than HTTPX's default.
The WHATWG-based URL parser/joiner still does not establish arbitrary HTTPX URL
construction, including malformed absolute Locations without a host.
Three additional source-only probes retain a percent-encoded hostname and
percent-encoded parent path segment, and repair `https:/next` to the previous
host. These are outside the passing URL corpus and identify follow-up targets
for the native WHATWG adapter; they do not certify corresponding wire behavior.

**High impact:** validating irrelevant assets could reject an otherwise installable
release. **Medium impact:** using the repository's ten-redirect policy rejects
release downloads that the source follows. **Medium impact:** sharing a metadata
cookie session with asset acquisition changes which cookies authorize the download.
**Medium impact:** applying the repository's per-hop downgrade check changes this
function's source-defined final-response policy.

For J JSON bytes, A assets and L aggregate name bytes, decoding and selection take
O(J + A + L) work plus numeric conversion costs and retain O(J + A) storage. Each
HTTP operation sends at most twenty-one requests, stores its own cookie state and
retains one complete decoded response. Decoded body storage is O(B) for B bytes;
cookie matching/sorting has the A65 bounds. No overall elapsed-time or downloaded
byte bound follows from the metadata size check or per-read deadlines.

The initial A66 all-target run passed 686 tests and failed one existing catalogue
assertion that expected Hy's old ambiguity/oversize messages. That assertion now
checks the source messages and retains its no-download/no-install checks. The
subsequent uninterrupted serial all-target run passed 687 tests: 260 unit tests
and 427 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. Both established source runtimes and the lint oracle were enabled,
using the full command recorded under A61. Corpus sizes are asserted in the new
tests. Rustfmt, whitespace checks, Clippy with warnings denied and Windows
all-target cross-compilation passed. The upstream checkout remained clean at the
pinned revision. QG3 and QG5 remain open for the recorded remaining contracts.

### Python JSON values and release metadata

Under A67, `src/util/python_json` supplies a typed JSON reader used by direct
GitHub release selection. The observed contract is `json.loads(response.content)`
inside the pinned `fetch_github_release_zip_asset`, using the A45 CPython 3.13.15
runtime. Its standard-library JSON implementation resides under
`/opt/homebrew/Cellar/python@3.13/3.13.15/Frameworks/Python.framework/Versions/3.13/lib/python3.13/json`.
The decoding oracle invokes that runtime's actual `json.loads`, including its C
accelerator, rather than reproducing JSON rules in the oracle.

Values distinguish null, booleans, arbitrary-precision integers, binary64 floats,
strings, arrays and ordered objects. NaN and positive/negative infinity remain
float values. They are not replaced by zero or string markers. Integer syntax
retains integer identity, while fractional/exponent syntax retains binary64 value
and signed zero. The shared default integer digit limit remains 4,300. Release
size comparison now uses exact integer comparison for integers and native IEEE 754
comparison for floats: NaN and negative infinity do not exceed the limit; positive
infinity does and displays as `inf` in the policy diagnostic.

The byte reader shares existing JSON encoding detection and implements
surrogatepass decoding for UTF-8, UTF-16 and UTF-32. Python strings are stored as
Unicode code points so unpaired surrogates are retained. Escaped UTF-16 surrogate
pairs combine into a supplementary code point; raw UTF-8 surrogate sequences retain
their separate code points, as Python does. Object keys compare these retained
values, and replacing a duplicate key preserves its original insertion position.
Names resembling serde's private number marker remain ordinary object fields.

Consumers perform conversion only where needed. Release ZIP suffix checks operate
on retained code points. Download URLs require a Unicode scalar string and reject
unpaired surrogates at that boundary. Asset names can contain surrogates without
preventing selection or installation. Diagnostic rendering escapes unencodable
surrogates as `\ud800`-style text, matching the observed source runtime's UTF-8
stderr with `backslashreplace`. Other stream encoding/error policies remain outside
that result.

Container parsing uses an explicit stack. Completed values are attached to their
parent array/object until the root is complete. Destruction also drains child values
iteratively, so a deeply nested ignored field does not merely postpone a Rust
stack overflow until the parsed document is dropped. No fixed nesting quota is
added. The first oracle run rejected the initial assumption that Python's default
recursion limit of 1,000 defines the JSON depth limit: the source accepted depth
1,100. A separate plain-json probe accepted 1,000, 1,100, 1,500, 2,000 and 5,000,
but raised RecursionError at 10,000 and 20,000. The exact boundary and its dependence
on runtime/call context remain unknown. The native stability test accepts and drops
depth 10,000; this is not claimed as source resource-failure equivalence.

Evidence:

- 2,012 byte-document comparisons cover literal spelling, whitespace, number
  grammar, arbitrary integers around the digit limit, binary64 rounding/overflow/
  underflow, escapes, surrogate pairs and unpaired surrogates, duplicate keys,
  malformed syntax, six UTF encodings/BOM forms, raw surrogate encodings and deep
  arrays/objects through 1,100 levels. Deterministic insertion/deletion mutations
  exercise punctuation and quoted input. Successful results compare flat,
  lossless projections: integer decimal strings, float bits, string/key code points
  and ordered container structure. Failures compare rejection, not exception text.
- The oracle scans every Unicode code point for lowercase mappings to `z`, `i`
  and `p`, confirming the represented ZIP-letter mapping uses only the corresponding
  ASCII uppercase/lowercase letters on Unicode 15.1.
- The existing release oracle expands from 666 to 732 documents under three tag
  states: 2,196 comparisons. New cases cover nonfinite values in selected and
  ignored fields, surrogate names/URLs, raw UTF-8/16/32 surrogates, deep ignored
  arrays, ambiguity diagnostics and ordinary private-marker-like object keys.
  Policy diagnostics are compared after the stated source stderr projection.
- Six new owned-server CLI scenarios combine three nonfinite sizes with UTF-8
  and UTF-16 metadata, surrogate names and ignored fields nested 1,100 levels deep.
  NaN and negative infinity install the expected plugin; positive infinity returns
  the source oversize message without downloading an archive or creating the plugin.
- Native regressions independently check scalar identity, arbitrary integers,
  reserved-looking object keys, surrogate preservation and iterative destruction.

The decoder and release oracles run with `-I -B` and filesystem-mutation audit
guards. Rust creates fixtures. This reader is adopted only for release metadata;
other JSON APIs, models, configuration and snapshot consumers retain their existing
contracts and require separate migration evidence. Exact syntax-error text,
runtime-adjusted integer limits, source C-stack exhaustion, allocation failures and
other-platform runtime behavior remain open. The new code does not close QG3 or QG5
for the full project.

**High impact:** rejecting an ignored surrogate/nonfinite field prevented an
otherwise valid release from installing. **Medium impact:** a small parser nesting
limit rejected source-accepted metadata; recursive destruction could recreate the
same failure after parsing. **Medium impact:** encoding Python nonfinite numbers
as ordinary JSON values loses the source's comparison and display semantics.

For B input bytes and N decoded nodes, decoding, syntax scanning, container assembly
and iterative destruction take O(B + N) work apart from numeric conversion and
expected hash-map operations. Integer conversion has an O(D²) upper bound for at
most D = 4,300 decimal digits; float conversion scans its input spelling without
that integer limit. Retained input/value/container storage is O(B + N), with explicit
parse/drop stacks bounded by the represented data. No new byte quota, elapsed-time
deadline or source-equivalent resource-exhaustion bound is implied.

The uninterrupted A67 serial all-target run passed 691 tests: 263 unit tests and
428 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. It enabled both established source runtimes and the lint oracle,
using the full command recorded under A61. Decoder and release corpus sizes are
asserted in their tests. Rustfmt, whitespace checks, Clippy with warnings denied
and Windows all-target cross-compilation passed. The source checkout remained
clean at the pinned revision. Full project parity remains open under QG3 and QG5.

### Automatic redirect target construction

Under A68, `src/util/http_redirect.rs` supplies target construction shared by
streamed API transfers and direct GitHub acquisition. Repository HTTP redirects
remain governed by the source's separate manual `URL.join` loop under A65.

The primary dependency source is HTTPX 0.28.1 in the A45 runtime:

- `_client.py`, especially `_redirect_url` and `_redirect_headers`, SHA-256
  `c43f941baefe58c91e96d00039e1868fe719d91453026d7db1647194563bff8d`.
- `_urlparse.py`, especially URL component parsing, `validate_path` and
  `normalize_path`, SHA-256
  `640987e3b38d7e4c6bae3f8f3d88467a21e36fa0232824be00d58837838bfca6`.

The shared helper follows the five automatic redirect statuses only when Location
is present. It combines duplicate Location values with comma-space using the
existing response-wide text decoder. The API previously selected only the first
value. Absolute spellings such as `https:/next` now receive the previous host;
the previous credentials and nondefault port are not copied. Explicit target
userinfo and ports survive URL construction, while the existing caller-specific
header policy determines whether Authorization is retained. Literal dot segments
are normalized before missing-host repair, matching the source's two-stage parse
and `copy_with(host=...)` sequence. Prior fragments retain the existing inheritance
rule. Non-printable ASCII characters and Locations above 65,536 Unicode characters
are rejected before redirect body consumption. Error text remains native.

Evidence:

- 480 HTTPX comparisons cross three bases, twenty Location arrangements and eight
  statuses. They cover credentials, nondefault ports, IPv6, explicit/missing hosts,
  duplicate headers, fragments, literal dot segments, absent headers and represented
  invalid inputs. The oracle invokes `_redirect_url` and `has_redirect_location`.
  An empty source URL path is projected to `/`, which HTTPX transmits on the wire;
  source string serialization without that slash is not claimed equivalent.
- The GitHub source-function corpus expands from 125 to 133 sequences. The eight
  additions compare actual HTTPX MockTransport requests, including Authorization
  retention/removal across repaired origins and explicit target userinfo.
- Two added CLI tests run four owned-server scenarios: API PUT-to-GET transfers
  and GitHub metadata plus archive acquisition, each using hostless targets or
  duplicate Location values. Assertions check exact request paths, Host headers,
  body/method transitions, successful confirmation or installation, and absence of
  unintended GitHub credentials.
- Native regressions cover the 65,536-character absolute-Location boundary and
  confirm that invalid Location values on nonredirect responses are ignored. Existing header-encoding,
  fragment, redirect-body ordering and redirect-limit regressions remain in use.

Bounded findings: **high impact** — transport still uses `url::Url`, which applies
WHATWG normalization to percent-encoded hosts and dot segments that HTTPX retains.
The source probes `https://%70lugins.hex-rays.com/final` and
`https://api.github.com/a/%2e%2e/final` therefore remain outside this result.
The initial Location character limit is not the complete source URL-size policy:
a source probe accepts an absolute ASCII URL of 65,536 characters but rejects a
relative path of that length after joining adds its authority. Complete joined-URL
and re-encoded component limits remain open; the native boundary regression uses
the absolute spelling, independently confirmed by that source probe.
**Medium impact** — host repair can change the effective port and origin, so tests
must observe Authorization and actual requests, not only URL strings. **Low impact**
— sharing the automatic policy removes drift between its two consumers without
changing the repository's distinct manual policy.

For L aggregate header/URL characters, construction and literal path normalization
take O(L) time and temporary storage, excluding downstream URL-library work. This
adds the source Location character limit; it adds no body-size or elapsed-time
limit. Complete URL grammar, malformed-input diagnostic equality and raw transport
equivalence remain open under QG3 and QG5.

Enabling `HY_TEST_HTTPX_ORACLE_PYTHON` for the full A68 run also exposed a
pre-existing indentation error in the inline environment-integer source fixture.
That script failed before comparison. Commit `eff650ac` corrects only its loop
indentation; all 78 environment-integer comparisons then passed. This does not
change runtime setting behavior or retroactively certify previously skipped oracles.

The subsequent A68 serial all-target run passed 696 tests: 266 unit tests and
430 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. It enabled the established bundle, venv and lint source runtimes
and additionally set `HY_TEST_HTTPX_ORACLE_PYTHON` to the A45 runtime. The final
absolute-Location boundary fixture adjustment passed a separate three-test redirect
rerun, including all 480 HTTPX comparisons. Logs are `/tmp/hy-redirect-full-final.log`
and `/tmp/hy-redirect-target-final.log`. Formatting, whitespace checks, Clippy with
warnings denied and Windows all-target cross-compilation passed. The source tree
remained clean at its pinned revision. QG3 and QG5 remain open for the documented
remaining contracts.

### GitHub catalogue retry and rate-limit handling

Under A69, catalogue acquisition separates HTTP setup (`github/http.rs`), retry
orchestration (`github/retry.rs`) and wait calculations (`github/retry/delay.rs`).
The source contract is `src/hcli/lib/ida/plugin/repo/github.py` at the pinned
revision: `_urlopen_with_retry`, `WaitGitHubRateLimit`, `_is_transient_error`,
`_is_rate_limit_error` and `_check_and_handle_proactive_rate_limit`. The installed
Tenacity version is 9.1.4, matching `uv.lock`. The oracle executes the actual nested
decorators rather than reconstructing their state machine.

The inner loop permits five attempts for HTTP 403/429. The outer loop permits four
attempts for HTTP 500/502/503/504 and represented connection/timeout failures.
Outer retries restart the inner attempt counter. Up to twenty logical send attempts
are possible when rate-limit failures precede each transient failure; redirects
within an attempt are separate network requests. Native automatic transport retries
are disabled so they cannot silently add attempts to this policy.

Reactive header precedence is Retry-After, reset timestamp, then exponential delay.
Retry-After uses Python integer syntax and is clamped to 60–3,600 s. Reset timestamps
also yield a delay clamped to that interval; the source implementation uses a
present reset header regardless of remaining quota, despite its narrower docstring.
Fallback rate-limit waits are 60, 120, 240 and 480 s before the final attempt.
Transient waits are 2, 4 and 8 s. The rate-limit wait strategy is evaluated even on
the fifth attempt, so malformed final headers still produce an error before retry
exhaustion is returned. Integer conversion retains the source 4,300-digit limit;
reset conversion overflow remains a failure rather than becoming a capped delay.

Successful responses with remaining quota at most two and a reset timestamp wait
proactively for max(reset − current time, 30 s), only when that result is below
3,600 s. Greater values cause no proactive wait. Remaining quota is parsed only
when both headers are nonempty; reset is parsed only when the quota qualifies.
Headers use urllib's first-field, ISO-8859-1 representation, rather than HTTPX's
combined-field and response-wide text decoding. Error and log wording remain native.

Search, GraphQL and remote catalogue archive requests now use this policy. Buffered
request methods, URLs, headers and bodies are cloned for each attempt. Bodies are
read and parsed only after the retry function returns, so JSON/ZIP validation and
body-read failures do not enter retry scheduling. Successful reads populate the
existing caches. Offline and cached paths keep their existing pre-acquisition
behavior; local file URLs retain the separate file acquisition path.

The catalogue HTTP client sends identity encoding and closes each connection,
preserves raw archive payload bytes, and has no added total request deadline.
The previous 60 s metadata deadline and repository response decoding do not model
the source's default urllib acquisition. The native Hy User-Agent remains explicit.
Archive GET redirects currently use reqwest's ten-hop policy; metadata still has
its existing no-follow policy. These are not claimed equivalent to urllib's
method, repeated-target and redirect-header rules, which remain separate work.

Evidence:

- 562 source sequences compare ordered requests, requested waits, simulated wall
  times and final status/error categories. They include mixed error types, attempt
  exhaustion, the twenty-attempt case, header duplicates and lazy parsing,
  malformed integers, overflow and digit limits. Read-only audit guards prohibit
  source-oracle filesystem mutations.
- A real refused loopback connection under Tokio's paused clock exhausts the
  production send adapter after exactly 14 s of scheduled waits (2 + 4 + 8 s).
  Other native regressions independently check counter reset and malformed headers
  on the last rate-limit attempt.
- The CLI recovery fixture retries both REST search queries, GraphQL POST and
  source-archive GET, preserving each method/body and credential boundary. A second
  invocation performs no network requests after the successful cache publication.
- Five CLI terminal cases cover 401, nontransient 501, invalid successful JSON,
  malformed reactive headers and malformed proactive headers. Each sends one
  request and leaves the catalogue cache unpublished.
- Two archive-encoding scenarios verify that ZIP bytes remain usable despite a
  gzip Content-Encoding header, while an actual gzip wrapper reaches ZIP validation
  unchanged and fails. Neither body-validation outcome triggers retries.

Bounded findings: **high impact** — nested limits cannot be flattened into a single
five- or four-attempt loop without changing recovery behavior. **Medium impact** —
performing decoding inside the retry loop would replay failures that upstream
does not retry. **Medium impact** — reqwest and urllib expose different network
exception classes; connection refusal is observed, but arbitrary write/reset/TLS/
proxy failures require further mapping. **Low impact** — cache formats and GraphQL
batching remain independent of retry scheduling and are not certified by these tests.

For A logical attempts and H header bytes, scheduling uses O(A × H) scanning work,
plus Python-integer conversion costs (quadratic upper bound in at most 4,300 digits).
A is at most twenty. Production scheduler storage is O(H) beyond the buffered
request, one response and its downstream body. Waits suspend the async task; no new
whole-operation deadline or response-byte quota is introduced. QG3 and QG5 remain
open for the listed acquisition, model, discovery and platform contracts.

The uninterrupted A69 serial all-target run passed 703 tests: 270 unit tests and
433 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. The A68 runtime configuration was retained, including the HTTPX,
bundle, venv and lint source-oracle environments. The retry corpus size is asserted
in its test. Formatting, whitespace checks, Clippy with warnings denied and Windows
all-target cross-compilation passed. Logs are `/tmp/hy-catalogue-retry-full.log`,
`/tmp/hy-catalogue-retry-clippy-final.log` and `/tmp/hy-catalogue-retry-windows.log`.
The source checkout remained clean at its pinned revision. This result does not
close QG3 or QG5 for full project parity.

### GitHub catalogue archive ordering and cache identity

Under A70, `github/acquisition.rs` owns archive planning. The primary contracts are
`GithubPluginRepo.get_plugins`, `get_release_asset`, `download_release_asset`,
`get_source_archive` and their cache helpers in the pinned
`src/hcli/lib/ida/plugin/repo/github.py`. Collection follows source `(owner, repo)`
tuple ordering; full-name string sorting would incorrectly put `a-b/r` before
`a/r`. All repository metadata is obtained before archive acquisition starts.
GraphQL batching remains separate work; this change establishes the acquisition
phase boundary without claiming equivalent metadata query shapes or counts.

Accepted distribution assets retain release order and asset order. All repositories'
assets are acquired before any source archives. Every accepted release contributes
its source archive, including repeated URLs. Eligible tags add a source only when
their URL has not already appeared in accepted releases or earlier eligible tags
of that repository. The source-URL set resets for each repository. Release/date,
asset type/suffix and tag prefix/date filtering remain source projections over the
represented metadata domain. The old sorted URL sets are removed.

Asset cache identity now consists of repository, release tag and asset name;
source identity consists of repository and commit. URL and size are not part of
those identities. GraphQL requests now ask for `tagName`, and the native release
model retains it. Metadata and archive cache resource keys use a new version so
old entries lacking tag identity or recording only URLs cannot supply ambiguous
bytes. Existing cache files are retained. Storage is still hashed and partitioned
by account and API origin; upstream path/format interoperability is not implied.
Old native entries need an online refresh to populate the new identities;
offline-only operation cannot reconstruct the omitted release-tag information.

The source checks its asset cache before applying the 104,857,600-byte download
limit. A cached asset can therefore remain eligible after metadata changes its URL
or reports a larger size. A cold oversized asset is skipped: the source getter
raises ValueError, which the catalogue caller catches. The initial review question
that oversized assets might terminate collection was falsified by this catch.
The native planner now retains those candidates until acquisition can consult the
cache; it no longer removes them during URL selection.

Rate-limit integer parsing now carries an explicit `GitHubValue` error category.
Archive acquisition skips that represented Python ValueError, matching the source
caller's catch. Metadata acquisition still propagates it. Timestamp overflow,
HTTP failures, fresh file-acquisition/cache-write failures and archive-indexing
failures propagate. Existing cache-read fallback remains native and unverified.
Indexing stays outside the skip boundary. This does not yet classify every Python
ValueError that urllib or file acquisition can produce, particularly URL/encoding
failures; those remain open transport work.

Evidence:

- 550 source comparisons execute `get_plugins.__wrapped__` with actual Pydantic
  metadata models and intercept its archive getters/index sink. They compare
  selected getter order, multiplicity, URLs and logical cache coordinates. The
  matrix covers release dates, content types, suffixes, the size boundary, repeated
  releases/assets, tag deduplication, shared commits/URLs and owner-name prefixes.
  These are collection projections, not cache filesystem or ZIP-model proofs.
- One CLI fixture covers two repositories with prefix-related owners. It verifies
  that both metadata queries precede downloads, assets precede sources globally,
  source URLs sharing a commit reuse bytes, and each plugin retains nine catalogue
  locations contributed by duplicate release and asset records.
- A three-phase CLI fixture expires only release metadata. Changing an asset's URL
  and reported size reuses its cached bytes; changing its release tag forces a new
  download despite an unchanged URL. Source bytes survive a URL change when the
  commit is unchanged. Snapshot URLs and versions reflect the new metadata and
  the appropriate cached or freshly acquired bytes.
- Four acquisition-failure scenarios verify skip behavior for malformed reactive
  asset headers and proactive source headers, while reset overflow and HTTP 404
  remain fatal. Successful cases retain two locations and continue collection.
- A later repository's metadata failure prevents every archive request. Existing
  cold-oversize, retry, compression, identity and cache-expiry tests remain enabled.
- All 562 A69 retry comparisons still pass after introducing the explicit
  acquisition ValueError category.

Bounded findings: **high impact** — URL-only cache entries cannot preserve release
or commit identity when metadata changes or different coordinates share a URL.
**Medium impact** — deduplicating release records changes catalogue multiplicity,
even when their bytes are read only once. **Medium impact** — tuple/string ordering
differences affect acquisition and failure order for ordinary owner names.
**Medium impact** — existing native caches require an online refresh after the
identity migration; this affects previously warmed offline workflows.
**Low impact** — versioned native cache keys retain older files, so this migration
does not reclaim their disk usage. Physical case/path aliases and upstream cache
layout remain outside the logical identity result.

For R repositories, N metadata records and L compared owner/name bytes, planning
uses O(R log R × L + N) ordering/traversal work plus string scanning/copying costs,
with O(N + B) storage for records and B retained input/planned string bytes.
URL-set operations are expected
constant time apart from hashing. Acquisition reads and indexes selected archive
bytes using the existing reader costs. No new metadata/archive quota or deadline
is added. QG3 and QG5 remain open for the stated remaining contracts.

The uninterrupted A70 serial all-target run passed 709 tests: 272 unit tests and
437 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. The A68 source-oracle environment configuration was retained.
Formatting, whitespace checks, Clippy with warnings denied and Windows all-target
cross-compilation passed. Logs are `/tmp/hy-catalogue-acquisition-full.log`,
`/tmp/hy-catalogue-acquisition-clippy.log` and `/tmp/hy-catalogue-acquisition-windows.log`.
The source checkout remained clean at its pinned revision. Full project parity
remains open under QG3 and QG5.

### GitHub catalogue GraphQL batching and partial results

Under A71, `github/graphql.rs` owns query construction and response envelopes;
`github/metadata.rs` owns cache warming and individual metadata lookup. The primary
contracts are `GitHubGraphQLClient.query`, `get_many_releases`, `get_releases`,
`warm_releases_metadata_cache`, `get_releases_metadata` and the constructor and
`get_plugins` method of `GithubPluginRepo` in the pinned
`src/hcli/lib/ida/plugin/repo/github.py`.

Warming probes each repository cache, retains misses in full-name string order
and queries successive groups of at most ten. Each query resets its aliases to
`repo0`, `repo1`, etc., shares `first = 10` and requests the upstream release,
asset, tag and default-branch fields. Subsequent per-repository lookup uses
`(owner, repository)` tuple order. The distinction is observable for `a/r` and
`a-b/r`. Existing metadata cache keys and the 86,400-second lifetime are retained.

GraphQL errors are checked before data or model validation. Any non-NOT_FOUND
record makes the response fatal; its diagnostic includes every such record in
source order. NOT_FOUND-only responses retain valid aliases. Missing/falsey aliases
and absent/falsey default branches are skipped with warnings. They are not
negatively cached, so ordinary lookup retries them individually, including on
subsequent invocations. Unrequested aliases are ignored.

All returned models in a batch are decoded before any of its cache entries are
written. A GraphQL or model failure therefore leaves that batch unpublished;
earlier completed batches remain cached. Cache writes themselves remain sequential:
a filesystem write failure can leave a written prefix. This is not an atomic
multi-file transaction. Existing corrupt/unreadable-cache fallback is native and
does not yet reproduce upstream's exception boundaries.

Evidence:

- 113 read-only comparisons execute upstream `get_many_releases`, intercepting
  its HTTP opener. They compare query fields and aliases after whitespace
  normalization, exact variables, selected repository names, failure outcomes
  and fatal GraphQL diagnostics. Query sizes include 0, 1, 2, 9, 10, 11, 20 and 21;
  the source method itself accepts more than ten, while the warmer chunks calls.
  Cases include missing/falsey aliases, invalid branches, extra aliases, invalid
  envelopes and mixed fatal/NOT_FOUND records. These are envelope projections,
  not complete Pydantic model-coercion or warning-presentation certification.
- One CLI test covers 0, 1, 9, 10, 11, 20 and 21 repositories, checks cold batch
  sizes, verifies zero warm-cache requests and deletes one owned cache entry to
  verify that only that repository is fetched again.
- A partial-result fixture checks survivor caching, repeated individual missing
  lookups and different full-name/tuple ordering across two invocations.
- A two-batch fixture injects either a fatal GraphQL envelope or an invalid model
  into the second batch. Recovery fetches both members of that failed batch while
  retaining the first ten cached repositories. Existing acquisition, retry and
  archive-cache tests remain enabled with the aliased request protocol.

Bounded findings: **high impact** — publishing models during response traversal
would preserve a partial failed batch and change the next invocation's requests.
**Medium impact** — negatively caching missing repositories would suppress source
retry behavior. **Medium impact** — full-name and tuple ordering differ for valid
owner prefixes, affecting individual lookup/failure order. **Low impact** — batches
reduce cold metadata request counts while fully cached invocations issue none.

For R selected repositories and M cache misses, warming performs R cache probes
and at most ceil(M / 10) queries absent retries. K aliases still missing afterward
cause K individual lookup queries. Ordering costs O(R log R × L), where L is the
compared identifier length; retained names require O(R + B) storage for B identifier
bytes. Query construction and response traversal scale with their represented
bytes; model validation, serialization and filesystem I/O add their own costs.
No new response quota or deadline is introduced. Full Pydantic coercion, Python
JSON edge grammar, cache path/format/read-error equivalence, urllib redirect/error
mapping, live GitHub and native Windows behavior remain open under QG3 and QG5.

The uninterrupted A71 serial all-target run passed 714 tests: 274 unit tests and
440 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. The read-only source-oracle configuration was retained. Clippy
with warnings denied and Windows all-target cross-compilation passed. Logs are
`/tmp/hy-graphql-batch-full.log`, `/tmp/hy-graphql-batch-clippy.log` and
`/tmp/hy-graphql-batch-windows.log`. Final readability-only fixture changes were
checked again with the affected unit/CLI suites and Clippy. Formatting and
whitespace checks passed. Full project parity remains open.

### GitHub catalogue cache read and expiry failures

Under A72, `github/cache.rs` returns separate hit, miss and error outcomes. Its
callers propagate filesystem, JSON and represented model-decoding errors instead
of treating them as misses. Warming still completes all cache probes before
issuing its first metadata query, so an unreadable later entry prevents requests
for earlier misses. The source contracts are `get_candidate_github_repos_cache`,
`get_releases_metadata_cache`, `get_release_asset_cache`,
`get_source_archive_cache` and their caller catch boundaries in the pinned
`src/hcli/lib/ida/plugin/repo/github.py`.

Existence checks use the shared CPython 3.13 pathlib policy. Absent paths,
non-directory parents and represented dangling/looping links are misses;
other stat/read errors propagate. The clock is sampled only after a metadata entry
passes its existence check and before its modification time is read; archive reads
do not sample it. Metadata age is computed by subtracting binary64 epoch timestamps.
Entries expire only when age is greater than 86,400 s; equality
and future modification times remain eligible. Expired metadata is unlinked
before returning a miss. Unlink failures propagate. Consequently, a failed refresh
leaves the expired entry absent rather than retaining stale bytes. Expiry precedes
JSON/model decoding. Archive bytes have no age limit, and archive read failures
also stop acquisition before any download attempt.

Evidence:

- 33 source-getter comparisons cross candidate metadata, release assets and source
  archives with eleven filesystem states: missing entry/parent, file parent,
  fresh/future/exact-boundary/expired file, current/expired directory, dangling
  link and symlink loop. Rust creates separate source/native fixture trees and
  checks native bytes, clock-read counts and deletion. The actual upstream getters perform reads;
  an adapter intercepts `Path.unlink`, records successful deletion intent and
  simulates directory-unlink failure. The read-only audit hook rejects any actual
  Python filesystem mutation. These comparisons establish represented outcomes,
  not Python exception-detail text or real source deletion syscall behavior.
- Three CLI corruption scenarios cover invalid candidate/release JSON and a
  release object missing required fields. They verify failure without network
  requests, output publication or modification of the corrupt entry.
- Three CLI filesystem scenarios replace candidate, release and archive entries
  with directories and verify failure without network fallback or replacement.
- Future-dated candidate, release and archive entries are reused without requests.
- Two failed-refresh scenarios expire invalid candidate or release JSON and
  return HTTP 401. Each issues one request, removes the stale entry before the
  failure and preserves the independent archive cache.

Bounded findings: **high impact** — swallowing cache errors changes command success,
network activity and whether corrupt user-visible cache data survives inspection.
**Medium impact** — clock skew can produce future modification times; rejecting
these entries triggers requests that upstream avoids. **Medium impact** — expiry
deletion persists even when refresh fails. **Low impact** — exact-lifetime equality
remains a hit; replacing the strict comparison with greater-or-equal changes that
boundary. Atomic native writes, hashed account/origin keys, upstream path/format
interoperability, malformed JSON/model grammar and concurrent filesystem behavior
remain independent work. A72 supersedes the read-error fallback limitation in
A70/A71 without certifying those remaining contracts.

For K cache-key bytes and B retained file bytes, key hashing and reading take
O(K + B) work and O(B) byte storage, plus path/stat/unlink I/O. Metadata decoding
retains its existing parser/model costs. Expired entries are removed without
reading their contents. No new byte quota, timeout or locking is introduced.
These results do not close QG3 or QG5 for full project parity.

The uninterrupted A72 serial all-target run passed 719 tests: 275 unit tests and
444 integration tests across 62 suites, with nine existing opt-in tests ignored
and no failures. All four read-only source-oracle environment variables were
enabled. A final refinement moved clock sampling after the existence check;
the extended 33-case source comparison, all 19 catalogue CLI tests, Clippy with
warnings denied and Windows all-target cross-compilation passed afterward.
Formatting and whitespace checks passed. Logs are `/tmp/hy-catalogue-cache-full.log`,
`/tmp/hy-catalogue-cache-source-final.log`, `/tmp/hy-catalogue-cache-cli-final.log`,
`/tmp/hy-catalogue-cache-clippy-final.log` and `/tmp/hy-catalogue-cache-windows-final.log`.
The upstream checkout remained clean at its pinned revision.

### Lint archive discovery, validation and README locations

Under A60, `src/cmd/plugin_lint/archive.rs` owns lint's archive policy and uses the
shared named ZIP reader. The old `scan_archived_plugins` function and `ArchiveFiles`
inventory were removed. Reference validation now uses `ArchiveReferences` everywhere;
the existing 38-case file-validation fixture exercises that inventory directly.
Lexical path display and README parent matching reside in `src/plugin/archive_paths.rs`.

The source contracts are `_lint_plugin_archive` and `_lint_readme_in_archive` in
the pinned `src/hcli/commands/plugin/lint.py`, together with
`validate_metadata_in_plugin_archive` in `src/hcli/lib/ida/plugin/__init__.py`.
Lint discovers every suffix-matching descriptor in central-directory order and
reads each by name, including duplicate records resolving to their last value.
It reports invalid models during discovery; member read and UTF-8 decoding failures
are terminal. It then validates references and reports recommendations for each
valid descriptor. A descriptor with missing references still counts as a valid
model, so reference failure does not produce a spurious “No valid plugins” finding.
Unrelated members are never opened and reference lookup does not inspect symlink
attributes or decompress referenced files.

README lookup compares lexical parents rather than raw string prefixes. Dot and
repeated separator aliases therefore follow pathlib, while parent components stay
lexical. Directory entries can satisfy README discovery, as in the source. An exact
`README.md` wins over earlier alternatives; otherwise the first directory-order
alternative is reported. Descriptor locations preserve their actual suffix-matched
filename and use native lexical path rendering.

Evidence consists of 181 native CLI/source-function comparisons and one direct
phase-order regression. The corpus includes 128 combinations of descriptor roots,
suffixes and README names; 32 descriptor-order/duplicate combinations; sixteen
member-fault placements; two Unicode-name/alias archives; a mixed reference/model
failure; and empty/invalid archives. It compares status and ordered reports,
including locations and recommendation explanations. Model-specific diagnostic
details are excluded; these results do not establish Pydantic error multiplicity
or text equivalence. Unix digest:
`bbd77afa9685de30a2fc6006c58d321cae788445372f20e29aab10b179bf4bc8`.
The source adapter invokes the actual archive function, uses `-I -B`, and installs
an audit hook rejecting filesystem mutation. All 181 comparisons also passed with
that hook active. The nineteen existing command-level source comparisons passed.

The uninterrupted serial suite passed 650 tests (244 unit, 406 integration) across
62 suites, with nine existing opt-in tests ignored and no failures. Rustfmt,
whitespace validation, Clippy with warnings denied and the Windows all-target
cross-check passed. Upstream remained clean at the pinned revision. The full run
used:

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
HY_TEST_LINT_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
cargo test --offline --locked --all-targets --no-fail-fast -- --test-threads=1
```

**High impact:** opening unrelated members made lint fail on otherwise inspectable
archives. **Medium impact:** eager reference validation changed finding order and
valid-descriptor accounting. **Medium impact:** raw prefix matching misidentified
README siblings and suppressed suffix-matched descriptors. **Low impact:** invalid
UTF-8 was previously downgraded to a model finding instead of terminating inspection.

For N members, B total name bytes and P valid descriptors, inventory construction
takes expected O(N + B) work and storage. README discovery takes O(P(N + B)) work;
each descriptor's temporary sibling-name list occupies at most O(B) storage. Model
storage and parsing/decompression costs are additional. Native Windows path/runtime
behavior, arbitrary model coercion, exact invalid-reference diagnostics, Rich markup
and terminal layout, filesystem races and all compressed-stream errors remain open.

### Installation descriptor selection and subtree extraction

Under A59, installation uses the shared named ZIP reader for both inspection and
staging. Selection resides in `src/plugin/manifest/selection.rs`; extraction resides
in `src/plugin/install/archive.rs`. Lexical archive paths reuse the existing pathlib
parser through `src/plugin/archive_paths.rs`. Lint's separate scanner was
subsequently replaced under A60, preserving its different reporting policy.

Named selection follows `get_metadata_from_plugin_archive`: it traverses descriptor
names in directory order, skips malformed descriptor data and returns at the first
exact matching plugin name. It does not validate file references or open unrelated
members. Unnamed direct installation follows the command's `list(...)` acquisition:
it reads all candidate descriptors before requiring exactly one valid record.
Duplicate descriptor names therefore still count separately, even though each named
read resolves to the final record. A damaged later descriptor can fail direct
acquisition while a preceding named match succeeds.

`ArchiveReferences` gives installation and repository indexing the same exact-name
membership policy. It uses the descriptor's lexical parent, including the source's
dot, repeated separator and anchor spelling, then applies the existing distribution
validation rules. It does not decompress entry points or reject symlink attributes
during reference lookup. Installation validates those references after selection
and before staging, preserving the source's separation of concerns.

Extraction follows `should_extract_plugin_archive_path` and `validate_archive_entry`
in the pinned `src/hcli/lib/ida/plugin/install.py`. Raw prefix selection and `.git/`
filtering precede POSIX relative-path conversion. Only selected entries participate
in the validation pass; local headers are opened during the subsequent copy pass.
Named reads preserve last-member lookup. Archive permission bits are not restored:
files receive their creation permissions, as with the source's `target_path.open("wb")`.
The existing staging owner still removes failed preparations and preserves an
installed version until publication. Native destination containment remains checked;
exact Windows drive/path behavior is not certified.

Evidence:

- 325 comparisons invoke the actual source metadata selection functions and
  `validate_metadata_in_plugin_archive`. They compare complete selected plugin
  metadata, extraction prefix, reference-validation status and selection failure.
  Sixty-five archive shapes are tested with five named/unnamed selectors, covering
  valid/invalid descriptor orders, repeated names, suffix matches, lexical roots,
  missing entry points, symlink attributes and damaged selected/later/unrelated members.
  Unix digest: `e31b3c282e461be4b3835b5e2feffb20e359f63708dbd4eb8e81aeca66e0afe6`.
- Ninety extraction comparisons call the actual source filtering and validation
  predicates, then read selected names through CPython ZIP. Successful directory
  inventories, file lengths and hashes match the Rust extraction result. Failures
  compare status only. This is an extraction projection, not execution of the full
  source staging/publication function. Unix digest:
  `b6a4efa2a1478f4c8c8b1f480308543e60a56f2ed9158f95c90f55f640aecb85`.
- Four CLI regressions check ignored unrelated traversal/symlink/corrupt members
  and filtered `.git/` contents; last-record payload bytes and duplicate-descriptor
  rejection; named installation before a damaged later descriptor; and Unix file
  creation permissions despite executable archive attributes. The existing rollback
  and dependency-preflight fixtures now place their rejected traversal inside the
  selected subtree, since an unrelated traversal is skipped by the source. The
  dependency fixture initially failed under its former whole-archive rejection
  expectation; all five dependency tests passed after correcting its scope.
  The invalid-descriptor CLI fixture likewise expects archive selection failure
  after invalid records are skipped; directory and snapshot inputs retain their
  field-level diagnostic assertions. Every case still requires failure before pip
  invocation or publication. The invalid-version upgrade fixture uses the same
  archive-selection diagnostic and still verifies the installed version and
  configuration are preserved.

The first full run encountered an intermittent OAuth body-limit test failure:
`read_to_string` returned `ConnectionReset` while waiting for TCP EOF. The unchanged
test passed on recheck. Its client helper now reads the declared HTTP Content-Length
and requires that entire body, following [RFC 9112 §6.3](https://www.rfc-editor.org/rfc/rfc9112.html#section-6.3).
A deterministic fixture accepts a complete response followed by a reset and rejects
a truncated body; the real body-limit test passed twenty consecutive repetitions.
The callback server and response-status assertions were unchanged. This corrects
the test's message boundary; it does not establish that every rejected request
always receives its complete response under arbitrary transport failures.

Final validation covers all 62 suites: 648 passed (244 unit, 404 integration),
nine existing opt-in tests ignored. This is the latest result per suite across
serial runs. The all-target run reached `plugin_manifest`; its corrected fixture
passed separately. The subsequent seventeen-suite run completed every remaining
suite using `--no-fail-fast`; its invalid-version upgrade diagnostic was corrected
and all seven upgrade tests then passed. The four new installation regressions
passed in the all-target run. Both source-comparison tests were rerun successfully
after formatting their reference scripts. Rustfmt, `git diff --check`, Clippy with
warnings denied and the Windows all-target cross-check also passed. Upstream stayed
clean at the comparison SHA.

The full-suite invocation used the following local oracle/toolchain configuration;
the remaining-suite and fixture reruns used the same relevant environment variables:

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
cargo test --offline --locked --all-targets -- --test-threads=1
```

All source oracles are read-only. Full source installation with dependency changes,
arbitrary metadata coercion, filesystem publication failures, compressed-stream
failure timing and local-source mutation between inspection and staging remain
outside these comparisons.

**High impact:** eager scanning made an unrelated descriptor failure prevent named
installation. **Medium impact:** filtering descriptors by file validity changed
direct-archive counts. **Medium impact:** global entry validation rejected usable
subtrees, while permission restoration differed from source-created files.
**Low impact:** stripping the descriptor suffix was not equivalent to taking its
lexical parent for noncanonical descriptor names.

For N members and B name bytes, name inventory construction takes O(B + N) expected
work and O(B + N) storage. Selection reads D descriptor bytes, stopping at a named
match or traversing every candidate for direct installation. Extraction scans the
directory twice in source order conceptually; the native validation pass retains
the selected names and output paths for its copy pass. Prefix/path processing takes
O(B + NP) work for prefix length P. Copy costs follow A58, with no new archive-wide
quota or deadline. These bounds exclude model validation and codec internals.

### Wheelhouse extraction and bundle ownership

Under A58, wheelhouse extraction resides in `src/plugin/bundle/wheelhouse.rs` and
uses `BundleReader`'s existing open archive. Installation and upgrade pass that
reader through dependency resolution. The redundant repository pathname field
and standalone manifest-reading wrapper were removed. Manifest selection and
wheelhouse bytes therefore come from the same opened bundle even when its pathname
is replaced; in-place mutation remains a separate unverified case.

The policy follows the pinned HCLI
`src/hcli/lib/ida/plugin/repo/bundle.py::PluginBundleRepo.extract_wheelhouse`.
It first selects entries under the requested prefix and excludes directories.
Validation applies to those selected records only, in directory order. Symlink
classification uses the high nibble of each record's external attributes,
independently of the creator OS and of the record used for a named read. Basenames
are flattened using POSIX component rules, and a repeated basename stops extraction
after retaining files already written. Duplicate raw names are retained; the first
encounter reads the final member's bytes before the second encounter reports a
duplicate. Bundle path validation now checks absolute POSIX paths, parent components
and backslashes, permitting the source's empty/dot/colon spellings. A native
destination-containment check still rejects a basename that changes the destination
root, including Windows drive-prefix cases; exact native Windows parity is open.

`Archive::copy_to` accepts a destination factory so local-header, filename,
encryption and codec checks precede destination creation, while data-read errors
occur after it. The shared decoder transfers bounded chunks and checks the final
chunk's CRC before writing it. Chunk sizes follow CPython 3.13.15 `shutil.COPY_BUFSIZE`:
65,536 bytes on Unix and 1,048,576 bytes on Windows. Whole-buffer reads use the same
path with a vector destination. The partial-file evidence below is for stored ZIP
members; compressed decoder read-ahead and failure timing are not certified by it.

Evidence:

- Ninety-three comparisons execute the actual HCLI extraction method. The source
  reads Rust-owned archive fixtures and writes to an in-memory destination adapter;
  no Python process edits filesystem files. Comparisons include success, destination
  creation, output filenames, byte lengths and SHA-256 values. The corpus contains
  seventy prefix/path combinations, twelve selected/unselected faults, four duplicate
  or alias layouts and seven CRC cases around the 65,536-byte copy boundary.
  Output inventories are sorted before digesting to avoid filesystem-order dependence.
  Unix digest: `10d66bca4c66b218e0e3820fd81e917773d542fc025a7eb14317e24f00fe8d19`.
- A native CLI regression installs from a bundle with an unrelated traversal path,
  symlink and damaged local header, verifies the selected wheel reaches the fixture
  pip process and checks installation publication. Existing collision, custom-source,
  target-selection, bundle-info and repository-loading tests also exercise this path.
- The Unix retained-file test replaces a bundle pathname, then checks both plugin
  and wheelhouse reads: the original repository retains the original bytes and a
  newly loaded repository sees the replacement.

The serial suite passed 641 tests (241 unit, 400 integration), with nine existing
opt-in tests ignored across 62 suites. The A57 source corpus and existing bundle
corruption comparisons still pass. Rustfmt, whitespace checks, Clippy with warnings
as errors and Windows cross-compilation passed. A final readability refactor of
missing-target reporting also passed its focused CLI regression. The source checkout
remained clean at the comparison anchor.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --all-targets -- --test-threads=1
```

**High impact:** validating unrelated outer members rejected usable bundles before
dependency resolution. **Medium impact:** reopening a pathname could combine a
plugin from one bundle with wheels from another. **Medium impact:** validating all
duplicates before writing changed source partial-output behavior. Remaining scope
includes lint archive selection and reporting, decoder failure timing,
real pip resolution and filesystem errors outside the in-memory source adapter.

For N directory records and B selected-name bytes, selection and basename tracking
take O(N + B) expected work and O(B) retained metadata. Copying U output bytes takes
O(U) work outside codec costs, with a fixed transfer buffer plus codec working
storage. Whole-buffer reads additionally retain O(U) bytes. No archive-wide quota
or extraction deadline is introduced. These bounds depend on A58.

### Ordered ZIP names and named reads

Under A57, `src/util/python_zip.rs` owns directory order and last-occurrence lookup.
Directory location, member records, filename decoding and decompression each occupy
a separate module. This replaces `ZipArchive` for shared repository indexing and
bundle recognition, manifests, member fetches and metadata selection. Installation
selection and extraction were subsequently migrated under A59, and lint under A60.
Wheelhouse extraction was migrated under A58.

The adapter follows [CPython 3.13.15's ZIP reader](https://github.com/python/cpython/blob/v3.13.15/Lib/zipfile/__init__.py), specifically
`_EndRecData`, `_EndRecData64`, `_RealGetContents`, `ZipInfo._decodeExtra`,
`_sanitize_filename` and `ZipFile.open`. Names remain in central-directory order,
including duplicates. Named reads resolve to the last matching effective name.
The distinction matters when raw names coincide but encoding flags differ, or when
Unicode path fields/NUL truncation alias distinct raw names. Local-header checks
use the original decoded name, before Unicode path substitution and sanitation.
The CP437 table follows [the same runtime's codec](https://github.com/python/cpython/blob/v3.13.15/Lib/encodings/cp437.py). Attribution and adaptation notes
are retained in `src/util/python_zip/LICENSE`.

Directory traversal uses the declared byte size rather than the entry count.
The reader handles concatenated archives and ZIP64 directory/member fields,
retains signed inferred header offsets until a read is attempted, and computes
overlap bounds in stable header-offset order. Header validation precedes encryption
and unsupported-codec rejection. Empty Unicode path fields and coincident header
offsets retain the source's data behavior, but Python warning output is not emitted.

The existing ZIP library supplies decompression through its stream API. A 51-byte
in-memory local header supplies authoritative central sizes and CRC, followed by
the original compressed bytes. No archive copy, archive rewrite, unsafe metadata
reuse or vendored dependency is required. The output is limited to the declared
uncompressed size and CRC-checked. Stored, DEFLATE, bzip2 and LZMA methods are
accepted; other methods fail as unsupported, as in the pinned Python runtime.
Malformed compressed-stream behavior beyond the tested corpus remains unverified.

Evidence:

- 1,025 CPython comparisons check ordered names, every named read's bytes and
  failure categories. They include 341 exhaustive short duplicate/encoding orders,
  all 256 CP437 bytes, invalid UTF-8, Unicode extra fields, NUL aliases, header/name
  mismatches, flags, CRC/size errors, overlap, extraction versions, entry counts,
  prefix/comment handling, truncations, ZIP64 records and all four supported codecs.
  ZIP64 fixtures exercise all eight combinations of large member fields, directory
  extension records, prefixes and damaged locators. LZMA payload bytes were generated
  with `xz --format=lzma --stdout`; archive fixtures are assembled by Rust.
  Non-Windows result digest:
  `26895d011a0359fb13444ace7a75331cb312752db387a745cc0ffb6720781cca`.
- Thirteen comparisons call the actual HCLI `PluginArchiveIndex` and compare
  complete serialized catalogues for duplicate descriptors, invalid JSON and
  Unicode/NUL aliases in both orders. Digest:
  `69a07a29a2f9be41155315da4e34d40692de3cc37b30e4fe363b7762691f508c`.
- Two CLI tests make ten source comparisons for duplicate inner descriptors,
  duplicate outer archives and duplicate bundle manifests. They compare success
  and complete report text, including valid/invalid first/last members.
- Existing A48 corruption and A56 repository acquisition/lifetime fixtures exercise
  the new shared reader. Exact stderr, exception classes within grouped failure
  categories, warnings and native Windows runtime behavior are not certified.

The final serial run passed 639 tests (240 unit, 399 integration), with nine existing
opt-in tests ignored across 62 suites. Rustfmt, whitespace checks, Clippy with warnings
as errors and Windows cross-compilation passed. The source checkout remained clean
at the comparison anchor. No Python process edited files; source oracles read
Rust-owned fixtures or standard input.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --all-targets -- --test-threads=1
```

**High impact:** losing duplicate directory records changes descriptor multiplicity,
last-value selection and catalogue contents. **Medium impact:** Unicode aliases and
encoding flags make raw-byte deduplication insufficient. **Medium impact:** decoder
size handling changes CRC outcomes and therefore whether an outer member is skipped.
A58/A59/A60 subsequently integrate wheelhouse, installation and lint reads.
These changes do not establish whole-project ZIP parity.

For N members and B directory/name bytes, parsing and lookup construction take
O(B + N) expected work; overlap-bound sorting adds O(N log N) comparisons. Retained
metadata uses O(B + N) space. Each named lookup has expected hash-table cost plus
name-byte hashing. A read retains O(U) output bytes, bounded by the member's declared
uncompressed size, and decoder-specific working storage. No new archive-wide quota
or deadline is introduced; these bounds exclude codec internals and filesystem I/O.

### Shared repository archive acquisition

Under A56, filesystem, GitHub and bundle repositories accumulate descriptors in
`ArchiveCatalogue`, then construct the final ordered catalogue once. They share
`src/plugin/index/archive.rs` for descriptor reading, file-reference validation,
host filtering and hash recording. The full manifest reader now belongs to the
plugin manifest module; bundle creation retains its existing metadata-only adapter.
This removes the second archive-grouping implementation and the separate bundle
descriptor-indexing loop.

The shared scanner follows `PluginArchiveIndex.index_plugin_archive`: malformed
descriptor data is skipped; archive/member-read errors propagate; invalid file
references stop the current archive after retaining earlier accepted descriptors.
File validation precedes expected-host filtering. An empty expected host imposes
no filter. Host normalization is delayed until a valid descriptor reaches that
step, so an invalid requested host does not reject an empty archive.

Filesystem loading follows `FileSystemPluginRepo.get_plugins` in the pinned
`src/hcli/lib/ida/plugin/repo/fs.py`: files precede child-directory traversal,
filesystem order is retained, directory symlinks are not recursively walked,
file symlinks are read through their lexical aliases and unreadable archives fail
the load. Directory-enumeration errors are skipped as by default `os.walk`.
The previous recursive-directory regression incorrectly expected a malformed ZIP
to disappear; it now checks failure before removing that file and checking valid
recursive discovery.

`BundleReader` retains an opened ZIP and its parsed manifest. Bundle inspection
and repository loading use this reader; indexing preserves outer member order and
`hcli-bundle:` locations. `LoadedRepository::fetch_verified` dispatches those URLs
to the owning bundle reader and ordinary URLs to transport, then applies the shared
checksum comparison. Installation and bundle creation use that method. Whole-bundle
eager extraction and temporary-file URLs have been removed; installation stages only
the selected archive bytes. A58 subsequently integrates wheelhouse extraction with
the retained reader and extends the lifetime test to cover wheelhouse bytes.

GitHub archive indexing now propagates scanner errors instead of converting them
to skipped-archive notes. This follows the source's `GitHubPluginRepo` acquisition
loops: their fetch-specific `ValueError` catches do not surround index insertion.
The native discovery, fetch retry/cache policy and cross-repository asset/source
acquisition order have not been certified by this change.

Evidence:

- 66 actual source `PluginArchiveIndex` comparisons: eleven archive shapes × six
  expected-host states. Successful complete serialized catalogues match; failures
  are compared as unsuccessful outcomes, not exact exception reports. Fixtures
  cover empty/non-descriptor archives, malformed JSON/UTF-8, missing entry files,
  partial indexing, foreign identities, duplicate descriptor ties and invalid ZIPs.
  Fixture ZIP timestamps are fixed for reproducible hashes.
  Digest: `109eb8759e7fd8e46014c18a42bbafd91ae9346b97db372fd0cb472acd49bc65`.
- Seven integration tests in `plugin_repository_loading` make eleven source
  comparisons: ten complete snapshot/failure projections against actual filesystem
  and bundle repositories, plus one source-verified bundle member fetch whose bytes
  are preserved in the resulting bundle. Unix-specific cases cover file/directory
  symlinks and the fixture interpreter used for bundle creation.
- A Unix unit test opens a bundle, replaces its pathname with another bundle and
  confirms that the original repository still reads its original open file; a fresh
  repository sees the replacement. A modified location hash is rejected with the
  original member URL. This test is native regression evidence, not a source oracle.
- A GitHub REST/GraphQL fixture returns an invalid asset archive and verifies that
  catalogue loading fails before requesting the next source archive. Existing
  GitHub, bundle-info, bundle creation/installation, snapshot and upgrade regressions
  also exercise the shared path.

**High impact:** silently omitting malformed local/GitHub archives concealed source
failures and could produce incomplete catalogues. **Medium impact:** canonicalizing
file symlinks or rewriting bundle URLs changed exported locations and tie ordering.
**Medium impact:** ZIP 8.6.0 retains central entries in an `IndexMap` keyed by raw
member name. A57 subsequently replaces it for these named-read consumers and tests
duplicate-name behavior. Permission races, special files, path encodings, re-index timing,
in-place mutation and native Windows behavior remain outside these fixtures.

Shared indexing reads/decompresses B selected bytes and retains O(P) accepted
descriptor bytes plus archive-name metadata. Catalogue ordering costs follow A55.
Filesystem acquisition holds one directory listing at each recursion level and
one archive buffer; retained directory entries depend on tree width and depth.
Bundle loading keeps one file descriptor and central-directory metadata instead
of extracted archive files. Fetching retains O(A) bytes for the selected archive
and hashes them in O(A) work. These bounds exclude decompression and arbitrary
metadata-validation costs; no new size quota or whole-command deadline is added.

### Bundle catalogue identity and ordering

Under A55, bundle inspection returns repository plugins through
`src/plugin/index/catalogue.rs`, rather than grouping case-sensitive names in the
command. The catalogue groups by lowercase name and normalized host, retains
version/compatibility insertion order, sorts version keys by semantic precedence,
and orders locations by URL and checksum. The last visited descriptor supplies
the display spelling. The command then sorts version labels lexically for its
report, as the source command does.

The source compatibility key is a tuple of two frozensets. Set ordering is a
partial order, so it does not satisfy Rust's comparison-sort contract. The isolated
`src/util/python_sort` implementation retains CPython 3.13.15's run detection,
binary insertion, powersort merge schedule and galloping comparison order. It
sorts references using safe slices and a separate merge buffer. It is adapted from
[CPython's list implementation](https://github.com/python/cpython/blob/v3.13.15/Objects/listobject.c),
with its attribution, adaptation summary and PSF license retained beside the code.
This is an explicit runtime-version assumption, not a guarantee for every Python
version supported by HCLI.

When URL and checksum tie, the source tuple comparison reaches the Pydantic
descriptor. Equal descriptors are accepted; distinct unordered descriptors fail.
Inspection now preserves the full descriptor, including the otherwise excluded
`$schema` field. A small equality adapter handles nested JSON fields and Python
bool/int/float equality, including exact integer-to-float comparisons above 2^53.
It does not certify nonstandard JSON or NaN behavior.

Read-only source comparisons cover:

- 5,461 exhaustive short sequences over four partial-order keys, lengths 0–6;
  digest `a7935c8e9c76b8542091493ee4c033c1a9727e8166e431555bf5a00b57122dd9`.
- 3,072 longer sorting cases: 12 lengths from 31 to 1,024, 64 seeds and four
  patterns; digest `6f180536e922fb84ea36179d680855a0a36f66f82b228948c136248c9bb1f0d2`.
- 725 catalogue identity/version/location cases, including all 720 permutations
  of six records; digest `134b49820c6a57886d37391baaa4549256a6bc791af268f32332f9f1f871513c`.
- 256 catalogue compatibility cases across eight sizes and 32 seeds, up to 105
  distinct compatibility keys; digest `298eb2d6980b4eaecba78e3dbcb26ade34f8da0e0835e049a538cb38cd6fca21`.
- 1,033 descriptor-equality cases: 16 × 16 values in four layouts and nine schema
  pairs; digest `d7457536503023a6dc1bdc2ee1b26f00c1f0757db65fe7d09fd96cca355374f8`.
- Three integration tests with 40 complete source/native CLI comparisons: ten
  display-name/version cases, all 24 permutations of four compatibility variants,
  and six duplicate-descriptor cases. The compatibility report digest is
  `15f66cd82eb0a1f974e402ba42d17d95c4f0d74e2e0c0a57a6c5d1ebd5d71828`.

The catalogue oracle validates locations and populates the source index, then
executes actual `PluginArchiveIndex.get_plugins`; it isolates formatting from ZIP
acquisition. CLI comparisons exercise actual `PluginBundleRepo` walking and the
source command callback over the same Rust-owned archives. These compare complete
stdout reports and success/failure, not exact exception text or source exit codes.
The complete bundle-info
suite now has 16 tests with 166 source comparisons, including A46–A48 cases.

**Medium impact:** name spelling can depend on version ties and compatibility
insertion order; replacing this with lexical name sorting changes visible output.
**Medium impact:** excluded schema fields and numeric equivalence affect whether
duplicate archive locations can be reported. Other native repository loaders adopt
this catalogue under A56; their acquisition policies have separate evidence there.

Sorting N references uses O(N log N) comparison/copy operations and O(N) auxiliary
space; the run stack is O(log N). Catalogue grouping and ordering add identity,
version, compatibility-set and URL comparison costs. Descriptor equality traverses
field bytes and uses arbitrary-precision integer conversion for numeric comparisons.
Its time is bounded by O(D²) for D-digit integer conversion, with O(D) numeric storage.
No new archive-size, model-depth or command-time limit is introduced.

### Bundle recognition and inspection

Bundle code now separates manifest models/validation, archive inspection and descriptor
reading. The module root exposes their existing API. Descriptor parsing is shared by
creation and inspection; each caller retains its own file-validation policy.

Recognition follows `src/hcli/lib/ida/plugin/repo/bundle.py::is_plugin_bundle_zip`:
the path must be a file with the exact `.zip` suffix and a ZIP member named
`plugin-bundle.json`. Manifest contents are validated only when opened. Inspection
walks outer and inner archives in member order, accepts all names ending in
`ida-plugin.json`, and skips malformed descriptor data. Following
`PluginArchiveIndex.index_plugin_archive`, a descriptor with invalid file references
stops indexing that inner ZIP, retains already accepted descriptors and leaves later
outer archives eligible. Installation's multi-descriptor scanner retains its own policy.

The `bundle info` command prints manifest fields before indexing plugins, matching
`src/hcli/commands/plugin/bundle.py::info`. An invalid inner ZIP therefore leaves the
manifest portion visible before the failure. Empty results print `plugins: (none)`.

Four integration tests compare fourteen outcomes with the actual source recognizer
and command callback, using read-only Python execution and Rust-owned ZIP fixtures.
They cover six suffix spellings, malformed/empty ZIPs, invalid manifests, directories,
invalid descriptors before/after accepted plugins, independent outer archives, unusual
descriptor suffixes, ignored outer names, lexical version ordering and partial failure
output. The source console uses width 10,000 without color, under A46.

```sh
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_bundle_info
```

Bounded findings: **medium impact**—continuing past invalid file references advertises
plugins the source omits. Manifest timestamp coercion/rendering now has separate
evidence under A47; complete Pydantic exception reports remain open.
Unreadable outer members have separate coverage under A48; case-varying plugin
identity and display selection are corrected under A55. Duplicate ZIP names and unusual path spellings
also remain outside these comparisons. These findings keep QG3 and QG5 open.

### Repository selection and installation metadata

Under A54, `src/plugin/index/selection.rs` separates identity lookup, stable version
ordering and location compatibility from repository loading and transport. Names
use Unicode lowercase matching; an empty host is unqualified. Qualified lookup
propagates normalization failures. Empty version specifications mean `>=0`, and
every version key is parsed before selection, including keys with no locations.
Index name, host and version keys need not agree with location descriptor fields.

The source comparison invokes `BasePluginRepo.find_plugin_from_spec` through actual
`JSONFilePluginRepo` instances. The identity matrix has 2,016 cases; the version,
descriptor, ordering, platform and IDA-version matrix has 5,880 cases. Expected
result digests are respectively:

- `835e385bbc9614989d36ad74508179daf941ba8711524e9bfdad06c12fb73543`
- `af30225339de7e11134ecd1a56edf9c3c47082c806df4d58517bb31b9acbd08e`

These compare exact successful URLs and normalized failure outcomes, not exception
text or failure phase. Six bundle CLI/source cases cover independent index and
descriptor identities. Two snapshot CLI/source cases establish that an invalid
version key fails before fetching an otherwise matching archive.

Installation now passes only the selected descriptor name to archive selection.
The common `LoadedRepository::fetch_verified` method checks the downloaded SHA-256; the archive
supplies the installed version and host. Named archive lookup uses the first exact
name match. Direct unnamed installation still requires a single plugin. The
internal preparation check compares repeated reads of the selected archive's own
metadata; it does not compare against repository-advertised version or host.

Nine installation cases compare native installation results with upstream
`fetch_compatible_plugin_from_spec` and `get_metadata_from_plugin_archive` over the
same files: eight name/case/version/host mutations and one duplicate descriptor.
This source oracle is read-only and covers acquisition, not source publication or
dependency installation. Source provenance is the pinned revision's
`src/hcli/lib/ida/plugin/repo/__init__.py`, `src/hcli/lib/ida/plugin/__init__.py`
and `src/hcli/commands/plugin/install.py`.

**High impact:** the removed index/archive consistency checks previously rejected
source-accepted installations. Installed-name conflicts still use the archive's
host. **Medium impact:** invalid unselected version keys can now fail a request,
matching upstream. Malformed later archive members, generic URL normalization,
catalogue-scan failure policies and full installation equivalence remain open.

For P catalogue entries, V version keys and L candidate locations, lookup and
selection take O(P + V log V + L) operations plus string/version comparison costs.
Retained candidate/version references use O(P + V) space. Download hashing retains
the existing O(B) byte buffer. These are structural bounds, not command deadlines.

### Snapshot text serialization and sorted-key round trips

`Snapshot::to_json` now implements the output contract of
`src/hcli/lib/ida/plugin/repo/file.py::JSONFilePluginRepo.to_json`, and the native
snapshot command prints its result with one trailing newline. The source first
serializes through Pydantic, parses that JSON through CPython, then calls
`json.dumps(..., sort_keys=True, indent=4)`. The dedicated native renderer sorts
every object by its original string keys, uses four-space indentation, keeps empty
containers inline and preserves array order. Strings use ASCII JSON escaping,
including control characters, DEL and lowercase UTF-16 surrogate pairs for
supplementary code points. The same string writer handles keys and values.

Number rendering reuses the existing Python numeric representation implementation,
including binary64 shortest-decimal rules, exponent spelling and signed floating
zero. Integer negative zero becomes zero. Overflowing floating-point values become
`null`, matching Pydantic's default serialization before the source's JSON reparse.
The final document uses the shared default 4,300-digit integer-limit check before
being returned to the CLI. This does not establish arbitrary input-JSON or model
coercion equivalence.

Two formatter tests compare complete outputs with the actual source `to_json`
function. A read-only adapter substitutes a real Pydantic `RootModel[Any]` for the
snapshot wrapper, isolating formatting from the repository envelope tested under
A52. One input string contains every Unicode scalar value:
1,114,112 code points − 2,048 surrogate code points = 1,112,064 characters.
Its compact result-vector SHA-256 is
`98048830ee35cc22d635ffcbd6d1c1f000f1c37a22f7b09d8260940b48513f04`.
The second corpus combines 22 scalar/string/number values with four nested layouts
and two empty containers: 22 × 4 + 2 = 90 cases. Its result-vector SHA-256 is
`627e679fb9d2851558ffc291fea75073892271b19c56cafcbc47401bf263b2e1`.

Three complete CLI comparisons invoke the actual source snapshot command through
Click's isolated runner. They compare status and every stdout byte, including the
terminal newline, for Unicode/control text, mixed integer/float/overflow values and
nested metadata with deliberately unsorted keys. These use the full repository
and descriptor models, not the formatter adapter.

The existing version-order test adds two snapshot exports and source reloads.
Both exported version dictionaries contain `1` before `1.0`, regardless of their
input order. The source then selects the archive under `1`. Thus snapshot export
can change an equal-precedence choice after re-import; preserving original version
insertion order in the emitted JSON would differ from upstream. The complete
snapshot integration suite now has six tests, 23 CLI invocations and 22 source
validation, selection or output comparisons.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy plugin::index::snapshot
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_snapshot
```

Bounded findings: **medium impact**—sorted output changes equal-precedence selection
after a round trip. **Medium impact**—generic JSON serialization preserves numeric
spellings or overflow values that the source normalizes through Pydantic and Python.
**Low impact**—indentation, escaping and key-order differences prevent deterministic
text comparison. Unrepresentable surrogate strings, nonstandard input constants,
deep/oversized documents, custom runtime limits, model coercion, exception output and
native Windows newline behavior remain outside A53's established results.

### Repository snapshot envelopes and complete descriptor serialization

Repository models now reside in `src/plugin/index/models.rs`, following
`src/hcli/lib/ida/plugin/repo/file.py::StaticPluginRepo` and
`src/hcli/lib/ida/plugin/repo/__init__.py::{Plugin,PluginArchiveLocation}`.
The plugin list, plugin host and archive checksum are required fields. Null or
non-string checksums fail while loading the document, before archive transport.
An empty string remains a valid model value and fails later during hash verification,
matching the source's distinction between schema validation and content verification.

Locations embed the existing `PluginManifest` rather than a partial descriptor
containing only plugin metadata. This validates the required
`IDAMetadataDescriptorVersion` and the optional `$schema` field, includes the version
when exporting a snapshot and excludes `$schema` from serialization. Archive-derived
locations construct a complete version-1 descriptor. The previous native export
omitted the version that upstream requires when reading that document.

One shared schema-version deserializer now serves repository, bundle, complete
descriptor and minimal legacy descriptor readers. JSON `1`, `1.0` and `true` normalize
to integer 1; strings, null and other values fail. Only the repository wrapper supplies
a missing-version default. Version maps use `IndexMap`, preserving document insertion
order before stable semantic-version sorting. This matters when distinct keys such
as `1.0` and `1` have equal precedence: the source chooses the earlier location.

`LoadedRepository::fetch_verified` centralizes archive download and exact SHA-256 comparison. Both
packaging and installation use it. Packaging then follows A50's descriptor naming;
installation follows the archive-name and metadata rules under A54. Required string hashes
remove the former branch that silently skipped verification for a missing hash.

Envelope comparisons mutate 11 fields with 15 missing/null/scalar/container values,
then add 11 root shapes and four unknown-field cases: 11 × 15 + 11 + 4 = 180.
They compare acceptance, normalized wrapper fields, required descriptor version,
schema-field exclusion and ordered version/location lists. Invalid outcomes are
reduced to a Boolean; full Pydantic error details are not certified. Result-vector
SHA-256: `ae52a2c3496604c7af632d670a5cae79f12b1e42623d34078e1a678e196aaa22`.
Two additional raw JSON documents preserve deliberately nonlexical version-key
orders. Their result-vector SHA-256 is
`fc6d1bd4aff6e469f4bc2f81dbab4cf144ae3866312d4a0fd2f4c6b87a9558b5`.

Five integration tests cover 20 CLI invocations. Ten malformed snapshots fail before
any owned-server archive request, pip invocation or bundle output. Four empty-snapshot
cases validate repository version defaults/coercions. One exported local snapshot is
accepted by the actual upstream reader and includes the complete descriptor version.
Two equal-precedence cases compare the source-selected URL and native embedded bytes,
then export and reload their sorted snapshots under A53.
One installation case rejects an uppercase hash before publishing plugin files;
the same source hash policy is independently exercised by A50. The other 19 invocations
also run source validation or selection over the owned snapshot files.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy plugin::index::models
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_snapshot
```

Bounded findings: **high impact**—optional hashes permit archive retrieval without the
verification required by upstream. **Medium impact**—partial descriptor serialization
produces snapshots that the source reader rejects. **Medium impact**—sorting version
keys before selection changes the selected archive for equal-precedence versions.
Full metadata coercion, nonstandard JSON and duplicate-key behavior, aggregate errors,
repository initialization and native Windows remain outside the established A52
evidence. Snapshot text rendering is covered separately under A53.

### Plugin reference syntax and bundle preprocessing

The shared parser now follows
`src/hcli/lib/ida/plugin/reference.py::parse_plugin_reference`. Reference syntax no
longer invokes metadata name validation: lookup names may contain spaces, dots or
Unicode text. The parser still rejects empty names, slashes in the resulting name
and repeated `@` separators. It rejects direct GitHub install URLs first, then
peels a matching repository scope before parsing the final host qualifier. Version
syntax requires an operator followed by `=`, but retains the remaining text without
interpreting its semantic version. Error text uses the shared Python `repr` renderer.

Scope matching preserves the source regex's newline behavior: its `.+` excludes LF,
and `$` permits one terminal LF outside the captured scope remainder. Unscoped
lookup names retain their trailing LF. Host qualifiers match only the source's
GitHub and Hex-Rays portal patterns. The dedicated matcher handles ASCII case and
the four additional letters accepted by CPython's case-insensitive ASCII ranges:
U+0130, U+0131, U+017F and U+212A. Pattern matching and normalization are distinct:
a long-s in the HTTPS scheme matches the pattern but fails Python's ASCII scheme
parsing. Normalization lowercases the accepted spelling and removes one terminal
slash; it does not collapse dot components or percent-encode Unicode identity text.
The public identity normalizer uses this path for supported host patterns; its
broader URL fallback remains separately incomplete.

Bundle preprocessing follows
`src/hcli/commands/plugin/bundle.py::_resolve_plugin_bytes`. Successful parsing
removes the configured repository scope and retains normalized host qualification.
A parse error preserves the original spec and clears host qualification. The
helper then requires the substring `==` and constructs its example from that
preprocessed value. It does not require a single standalone equality clause.
Consequently, a parsed scope can coexist with an explicit bundle repository, and
matching compound specifications reach version selection. Native downstream version
validation still has the numeric and error-format limits recorded elsewhere.

Source comparisons cover:

- 7 prefixes × 10 names × 12 version strings × 14 host forms × 3 endings = 35,280
  syntax/error cases. Result-vector SHA-256:
  `104520347f2268e3bc42322869fddee09f167268e0b373e1ccea6bef903317b6`.
- 5 schemes × 4 authorities × 10 paths × 5 endings × 2 direct/qualified forms =
  2,000 host cases. Result-vector SHA-256:
  `2ba861e01ede0714dc014db7addec973aec7e6fce5969b9c9f228ccb7590cf1d`.
- 5 prefixes × 6 names × 8 version strings × 5 hosts × 2 endings = 2,400 bundle
  preprocessing cases. Result-vector SHA-256:
  `9473303e1ba2905338bde865347f4146f38b8ed4464b2c306ff2d88e93170adf`.

The first two corpora execute the actual source reference module and compare full
parsed objects or exact errors. Successful host-qualified cases also check the
shared native identity normalizer. The bundle corpus executes the actual helper
with an in-memory path adapter that always reports a nonlocal input; A49 separately
covers real local paths and home expansion. A repository adapter records the
forwarded spec/host before downstream version matching.

Two additional CLI tests compare ten outcomes: six successful scope/host/compound
specifications and four missing-equality diagnostics. These use the actual source
base repository selection over a fixture inventory, then its fetch verification and
creation loop. The earlier A50 cases now use that same base selection implementation.
Successful commands fetch all three canonical target platforms; early preprocessing
errors perform no archive fetch, create no output and invoke no pip download.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --bin hy reference
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_bundle
```

Bounded findings: **medium impact**—retaining a bundle scope rejects inputs that the
source resolves through its selected repository. **Medium impact**—URL transport
normalization changes dot-bearing identity paths before lookup. **Low impact**—
metadata validation applied to lookup names changes parsing/error precedence.
Complete legacy version-spec errors, arbitrary-size version matching, general
URL normalization, repository setup/loading, full snapshot validation and native
Windows invocation remain open under A51.

### Repository bundle fetches, grouping and descriptor naming

Repository packaging now follows the distinct contracts in
`src/hcli/lib/ida/plugin/repo/__init__.py::BasePluginRepo._fetch_and_verify`,
`src/hcli/commands/plugin/bundle.py::create` and
`src/hcli/lib/ida/plugin/__init__.py::get_version_from_plugin_archive`:

- Each selected platform fetches its distribution, even when an earlier platform
  selected the same URL and checksum. The computed lowercase SHA-256 must equal
  the advertised string exactly; uppercase, empty and incorrect strings fail.
- The fetch stage verifies bytes and retains the advertised plugin name. It does
  not apply installation's embedded host/version/entry-point checks.
- A shared archive collector retains first-seen hash order and records platforms
  for each result. Repeated hashes replace the stored name/bytes without changing
  their position. A single unique archive has no platform suffix.
- After the spec's platform fetches finish, version naming scans descriptors and
  returns the first exact-name match. Differently cased names do not match, and a
  later descriptor cannot replace the first match's version. A failure during a
  later fetch precedes an earlier archive's missing-name error.

The collector is shared by local and repository resolution. Its input type holds
fetched name/bytes separately from `ResolvedPluginArchive`, so callers cannot
represent a pending version with an empty placeholder. The existing 128-case local
read-order oracle now exercises this shared grouping implementation and retains
its A49 digest. Descriptor naming reuses the existing lazy descriptor reader.

Four integration tests add 14 CLI/source comparisons. Eight descriptor cases cover
host/version differences, missing entry points, empty platform declarations,
duplicate names, an unrelated earlier plugin, differing name case and an empty ZIP.
Four checksum cases cover valid lowercase, uppercase, empty and incorrect values.
The ordering case deliberately makes SHA-256 sort order differ from platform order;
it verifies repeated HTTP reads, platform suffixes and dependency arguments for all
three pip downloads. The final case checks a later hash failure before an earlier
missing-descriptor failure. Successful fixtures preserve embedded bytes exactly.
Failure fixtures publish no bundle and invoke no pip download.

Source execution reads Rust-owned ZIP fixtures. An in-memory adapter supplies the
repository inventory and transport bytes; actual source selection, verification, resolution-loop
statements and descriptor/dependency helpers determine expected outcomes. Source
hash errors are reduced to `hash mismatch` in this comparison, so full exception
rendering is not certified. Native tests use an owned local HTTP server and a fake
interpreter. The prior two-case bundle regression was corrected: the source rejects
the bad hash but permits the embedded host difference during packaging.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_bundle repository
```

Bounded findings: **high impact**—reusing installation validation rejects repository
archives that upstream can package. **Medium impact**—hash sorting changes dependency
order, and caching a download suppresses later transport/hash failures. **Medium
impact**—parsing descriptors during each fetch changes which failure is reported.
Required snapshot fields and missing/null checksum rejection are now covered under
A52; broader version grammar, parent-repository loading and selection remain open.
Native code also resolves all input specs before staging their dependencies,
whereas the source stages each spec before resolving the next. These limits and
unverified native Windows/live transport keep full parity open under A50.

### Local bundle source paths and read order

`src/util/python_path.rs` now owns the existing lexical path and home-selection
implementation. The pip find-links adapter calls its URL-preserving entry point;
the bundle source adapter calls direct path expansion. This keeps POSIX/Windows
path rules in one module while making the callers' different policies explicit.
No filesystem canonicalization is introduced. Source behavior comes from
`src/hcli/commands/plugin/bundle.py::{create,_resolve_plugin_bytes}` at the pinned
upstream revision.

The creation loop checks the original spec before choosing its read count. A
literal existing `.zip` is resolved once. A tilde path found only by the helper's
expansion is resolved once per distinct, sorted target platform. The local adapter
retains this distinction, hashes each result and keeps archives in first-seen
order. Equal content produces one archive without a platform suffix. Different
content retains the platforms that selected it. Local selection uses existence,
so a directory ending in `.zip` reaches the file read and fails there. Errors
about missing descriptors retain the original spec, including tilde/dot spelling.

The shared-path suite adds 21,120 comparisons with CPython's direct
`Path.expanduser` behavior over POSIX and Windows lexical fixtures. Its compact
result-vector SHA-256 is
`48fdf06f98366e520d149e83f351043ebcedb8f32ab9cb3a6b8962c0b92446af`.
The existing 21,120 find-links cases, 1,200 Windows home-selection cases and host
account comparisons remain in the moved suite with their original expectations.

The local read-policy test executes the source creation loop with in-memory
read/path adapters: 2 literal-path states × 4 platform lists × 4 byte sequences ×
4 failure positions = 128 cases. It checks read calls, first-failure termination,
archive order and assigned platforms. Its compact result-vector SHA-256 is
`cdbb40e29a228f9c20e68407db52b132f2320938b7ef5657b434dac52451d64b`.
Four CLI tests add 15 actual source-helper comparisons: eight successful path
spellings, three directories, two original-spelling diagnostics and two unresolved
home names. Successful cases build for two platforms and verify a single embedded
archive with unchanged bytes. These tests operate on owned temporary files only.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy python_path
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy local_read_order
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_bundle_dependencies paths
```

Bounded findings: **medium impact**—sharing pip's URL exception with local bundle
paths bypasses required home expansion. **Medium impact**—using `is_file` for
selection incorrectly sends `.zip` directories to repository resolution.
**Low impact**—collapsing platform reads can change results for mutable input
archives. Native expansion and existence are observed once before repeated reads;
the source observes them in every helper call. Concurrent path/account changes,
native Windows, non-UTF-8 paths, repository resolution and full exception rendering
remain outside A49's established results.

### Bundle member failures and lazy ZIP reads

Inspection now selects outer names before opening entries, following
`src/hcli/lib/ida/plugin/repo/bundle.py::_load_plugins_by_walking`. Only members
beginning with `plugins/` and ending with `.zip` are read. The member adapter skips
missing entries, invalid local headers and CRC failures, corresponding to the source's
`KeyError`/`BadZipFile` boundary. Unsupported compression, encryption and DEFLATE
decoder errors propagate. CRC classification checks the ZIP library's exact error-kind
and message pair; a generic `InvalidData` check would incorrectly suppress unrelated
decompression failures. Read errors in the manifest or an inner descriptor remain
terminal, preserving their different source exception scopes.

ZIP 2.4.2 inspected every local header while constructing the archive. A damaged
unselected entry therefore failed recognition or manifest reading before native code
could apply the source filter. The project now uses the maintained
[ZIP 8.6.0 reader](https://docs.rs/zip/8.6.0/src/zip/read/zip_archive.rs.html), which
loads central-directory metadata and opens local entries on demand. The previous
codec feature set is explicit in Cargo.toml. A57 subsequently moves named-read
consumers to an isolated directory adapter while retaining this library's stream
decompressor; installation and extraction still use its archive reader.

Inner descriptor discovery also tests the central name before opening a member.
Reference validation shares the existing distribution rules but supplies exact ZIP
member-name membership, following `does_plugin_path_exist_in_plugin_archive`.
It does not decompress referenced entry points or unrelated files. A member named
`plugin.py/` or `./plugin.py` does not satisfy a reference to `plugin.py`; a member
with symlink attributes and the exact required name does. Installation retains its
separate inventory and extraction policy. Local bundle construction uses the same
descriptor reader and preserves the input archive bytes even when unselected
entry-point contents cannot be read.

Seven added integration tests cover 81 CLI invocations and source comparisons.
Five damage classes alter CRC, local-header signature, compression method,
encryption flag or DEFLATE stream. A sixth shape combines encryption with an invalid
local-header signature and checks header failure before encryption rejection.
Tests place each fault in selected outer members,
ignored outer members, referenced/unrelated inner files, the manifest and inner
descriptors; they also check continuation into a healthy later member. Six cases
compare exact path spelling and symlink attributes. Nine cases combine stored,
DEFLATE and bzip2 members at both ZIP levels. Six creation cases call the actual
source local resolver, verify embedded bytes and compare subsequent info reports.
The comparisons check exit success and complete plain-text output, including which
manifest fields are printed before terminal errors. They do not certify identical
outer exception text. LZMA and the remaining codec/ZIP-format combinations are not
certified by this fixture matrix.

```sh
DEVELOPER_DIR=/Library/Developer/CommandLineTools \
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_bundle_info corruption
```

Bounded findings: **high impact**—eager local-header validation rejects bundles the
source can inspect and prevents name-only descriptor resolution. **Medium impact**—
catching every archive error hides unsupported/encrypted data that the source rejects.
**Medium impact**—normalizing member names before membership checks advertises missing
file references. Local/central filename mismatches, overlap detection, duplicate ZIP
names, metadata encoding and complete compression/error-policy equivalence remain
open under A48; A57 adds bounded evidence for named-read consumers. A final validation run encountered missing files in the selected
Xcode installation; per-command `DEVELOPER_DIR=/Library/Developer/CommandLineTools`
selects the verified installed toolchain without changing system configuration.
An unrelated connection-failover fixture exposed a relinquished-port race after a
connection-reset failure. It now uses an invalid IPv6 link-local interface scope to
produce a connection failure without releasing a TCP port; production retry policy
is unchanged. The fixture passes on the exercised macOS host. The specific cause of
the original reset was not observed, so port reuse is an inference, not a proven event.

### Bundle manifest datetime and version coercion

Manifest loading now validates and normalizes `builtAt` through a dedicated serde
adapter. Creation retains the source's second-resolution UTC `Z` spelling. Reading
uses the parser and conversion policy from the locked Pydantic runtime; reports use
Python `datetime.isoformat()` spelling, including `+00:00` for UTC and six fractional
digits when microseconds are nonzero. Naive values remain naive. Failed datetime-string
parsing retries date parsing and uses the retry's error, as the source validator does.
Date-only values become naive midnight. Year zero is rejected after parsing.

The adapter uses speedate 0.17.0, the same Rust dependency pinned by
[pydantic-core 2.41.5](https://github.com/pydantic/pydantic-core/blob/v2.41.5/Cargo.lock).
Its lexical-parse-float 1.0.5, lexical-parse-integer 1.0.5 and lexical-util 1.0.6
dependencies are also retained in Cargo.lock. Conversion follows the primary
[JSON input adapter](https://github.com/pydantic/pydantic-core/blob/v2.41.5/src/input/input_json.rs),
[datetime conversion helpers](https://github.com/pydantic/pydantic-core/blob/v2.41.5/src/input/datetime.rs)
and [lax date fallback](https://github.com/pydantic/pydantic-core/blob/v2.41.5/src/validators/datetime.rs).

Numeric strings, JSON integers and JSON floats retain their separate source paths.
Timestamp units use the source's inferred seconds/milliseconds rule. JSON integers
with more than 18 digits receive a datetime type error, including values that fit
in signed 64-bit storage: the source's
[Jiter number decoder](https://github.com/pydantic/jiter/blob/v0.11.1/crates/jiter/src/number_decoder.rs)
classifies those values as BigInt, which its datetime adapter does not accept.
The manifest's `Literal[1]` version accepts `1`, `1.0` and `true`; it rejects `"1"`,
other numeric values and unrelated JSON types.

The read-only oracle extracts the actual manifest classes and duplicate-target
validator from the pinned HCLI source, then executes Pydantic's JSON validator with
the lockfile's Pydantic 2.12.5 and pydantic-core 2.41.5. A 32,470-case corpus covers
calendar validity, separators, offsets, fractional precision, naive/aware values,
numeric thresholds, year bounds and wrong JSON types. Both normalized values and
field-error types/messages match; the result-vector SHA-256 is
`8cedc867bd1b317a5de02e81410b4fc89b599005155cfefd01a5f7a180d6caa8`.
Two additional CLI tests perform 31 comparisons with the actual source `bundle info`
callback, exercising the serde adapter, literal version equality and rejection before
any report text. The A46/A47 fixtures account for 45 CLI/source comparisons.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy bundle_datetimes
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_bundle_info
```

Bounded findings: **medium impact**—unchecked timestamp strings previously allowed
invalid manifests and exposed input spelling instead of normalized datetimes.
**Medium impact**—the pinned parser treats JSON `-1.25` as
`1969-12-31T23:59:58.250000+00:00`, but the string `"-1.25"` as
`1969-12-31T23:59:58.750000+00:00`. Tests retain this source behavior rather than
substituting conventional epoch arithmetic. **Medium impact**—bundle display-name
selection still depends on semantic-version order, partially ordered compatibility
sets and archive URL/hash order; lowercasing only the report key cannot reproduce
that source algorithm. This turn identifies that dependency without claiming it fixed.
Complete JSON-parser and aggregate model-error equivalence remain open under A47.

### Pip availability and bundle dependency policy

Pip availability now builds its own `python -c "import pip"` command with inherited
stdin, environment and cwd. It captures both streams as bytes and applies the source
default timeout of 10 s. Exit status zero reports availability; launch failures,
timeout, nonzero status and signal termination report false. Captured output is not
decoded, so malformed UTF-8 or NUL cannot trigger the version probe's text-error
policy. Installation and creation continue to use the same availability result.

The source recorder executes `src/hcli/lib/ida/python/__init__.py::has_pip` and
checks exact argv/options. Sixty-four cases combine four return statuses with four
stdout and four stderr shapes. Every case also runs an owned Unix shell executable;
the signal case terminates that executable with SIGTERM. The compact result-vector
SHA-256 is `79caf3f49ab82a9fcafdf2382fcf34b0d3b4d7016cf501aaa0476e11a35fe887`.
A separate native test checks missing, directory and nonexecutable paths plus the
actual timeout and termination of its owned process. Two CLI tests cover five
invocations, demonstrating stdin shared in order by pip import, version and the
requested child, inherited environment, ignored import output, and checked/skipped
installation diagnostics. The mandatory pip check is retained.

Bundle creation no longer invokes the installation dependency resolver. The source
`src/hcli/commands/plugin/bundle.py::create` collects only dependency values that are
explicit lists, preserving their order and duplicates. An inline-only bundle can be
created without selecting Python or reading its entry-point script. Later installation
still reads inline script metadata and supplies those dependencies to pip.

The new bundle descriptor reader follows the source
`get_metadatas_with_paths_from_plugin_archive` discovery rule: names ending in
`ida-plugin.json` are candidates, malformed descriptors are skipped, and ZIP read
errors propagate. It does not require entry-point files to exist. The source local
resolver `_resolve_plugin_bytes` accepts exactly one valid descriptor and preserves
the archive bytes. Local creation now uses that boundary instead of installation
file validation. The source's subsequent archive index may ignore an invalid plugin
without stopping bundle creation; this does not imply the resulting plugin installs.
The now-unused archive dependency resolver and file-validating convenience reader
were removed rather than retained as dead alternative paths.

Four CLI tests cover eleven invocations. Cases include valid/malformed/missing inline
scripts, a native entry point containing non-UTF-8 bytes, mixed explicit/inline archives,
duplicate requirements, unusual descriptor suffixes, skipped invalid JSON, multiple
descriptors, absent descriptors and later installation of an inline-only bundle.
Eleven read-only comparisons call the actual upstream local resolver and metadata
reader on Rust-owned archives. Those comparisons use the existing extension runtime
with Pydantic 2.13.5; they do not establish exhaustive coercion parity with the upstream
lockfile's Pydantic 2.12.5. The pip helper oracle uses the locked runtime separately.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy pip_availability
cargo test --offline --locked --test python_pip_availability
HY_TEST_BUNDLE_ORACLE_PYTHON=/tmp/hy-extension-runtime/bin/python \
  cargo test --offline --locked --test plugin_bundle_dependencies
```

Bounded findings: **medium impact**—sharing installation validation with bundle
construction rejects archives the source permits packaging. **Medium impact**—
resolving inline dependencies during bundle creation changes interpreter/network
requirements and failure timing. **Medium impact**—pip-import output can be invalid
text even when pip is available, so text decoding changes a status-only check.
**Low impact**—metadata discovery uses a suffix test rather than an exact basename.
These conclusions depend on A44/A45 and leave repository/model/ZIP edge cases open.

### Shared version subprocess behavior

Version probing separates command construction, result policy and text decoding.
It uses the source's version-only Python snippet, inherits environment/cwd/stdin,
and captures stdout/stderr under a 10 s deadline. It no longer uses the general
output wrapper that forces null stdin. Launch failures and deadline expiry return
no version. An owned Unix fixture verifies missing/nonexecutable paths, the actual
deadline, and termination of the timed-out interpreter process.

Both streams are strictly decoded as UTF-8 and universal newlines are translated
before exit status is inspected. A nonzero status with decodable output returns no
version. Valid stdout is stripped using Python whitespace rules; a nonempty result
is retained without imposing version syntax. A BOM remains part of the text.
Invalid stdout or stderr raises the source decoding diagnostic even after a
nonzero exit. Diagnostics retain the offending byte/span, offset and reason.

The result type distinguishes an unavailable observation from a decoding failure.
Current-version selection, doctor, creation and PATH candidate lookup propagate
decoding failures. Virtualenv lookup does not fall back to pyvenv.cfg after such a
failure; doctor's recommended-environment notes propagate it too. Advisory execution
checks and installation diagnostics instead skip failed state collection, as their
source exception handlers specify. Installation still performs its independent
mandatory pip check. Explain propagates earlier virtualenv/final-version failures,
while mismatch collection discards partial results and records a UnicodeDecodeError
in its dedicated error field.

Primary sources are the pinned `src/hcli/lib/venv.py::{probe_python_version,
get_virtual_env_version}`, CPython 3.13.15 `subprocess.Popen._translate_newlines`,
the state/guard/report collectors in `src/hcli/lib/ida/python/environment.py`, and
their doctor and venv-create callers. The oracle executes the source probe with an
in-memory subprocess adapter and CPython's real text translator. It checks the
exact argv and options: captured text, checked status and timeout=10.0 s, without
stdin/environment/cwd overrides. It does not run pip or edit files through Python.

The text corpus compares 86,279 inputs: empty input, all 256 one-byte and 65,536
two-byte inputs, 20,480 selected three/four-byte sequences after an ASCII prefix,
and six Unicode/newline strings. Its compact result-vector SHA-256 is
`379b57f3f5d1a4ec5a594b0c16c3d4916a86ac0a107a1993c2ba022dc58de91f`.
The source probe corpus compares eleven stdout shapes × eleven stderr shapes ×
two status outcomes, totaling 242 cases. Its result vector hashes to
`526175975c4adb4c561fa65c5da2d046258b63491a2efd0cfe0ccaa1f1009efa`.
Both deterministic digests remain in the default tests after source comparison.

Seven CLI tests cover 22 invocations. They verify stdin shared between the probe
and requested child; decode/status precedence in doctor, explain and bundle
creation; guard recovery and independent pip validation; preservation of an existing
environment; PATH lookup stopping before another interpreter; explain's recorded
mismatch error; and prevention of virtualenv configuration fallback after malformed
interpreter output.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::checks
cargo test --offline --locked --test python_version
```

Bounded findings: **medium impact**—a failing process can raise a decoding error
before its return code is handled. **Medium impact**—turning that error into an
absent version changes creation fallback and diagnostic behavior. **Medium impact**—
guard exception handlers permit installation to proceed after failed diagnostic
collection but retain pip availability checks. **Low impact**—inherited stdin is
observable even for a version-only command. These conclusions depend on A43's
UTF-8 assumption; native Windows, alternate locales and descendant/cancellation
behavior remain outside this evidence.

### Python explain reports

Explain-environment separates typed records, installation observations, Python
observations, note policy and rendering. Collection follows source order: selected
installation gates the remaining sections; environment resolution precedes
embedded-venv inspection, independent final-interpreter version collection,
mismatch checks and notes. Successful overrides supply no IDA probe. Failed
resolution can retrieve an available probe, and report errors include mapped
exception-class labels. Other OS/model exception boundaries remain open.

Embedded-venv details use the probe's VIRTUAL_ENV, not the selected executable's
layout. Configuration parsing shares case-insensitive keys and last-value precedence;
the runnable interpreter's version precedes recorded version/version_info. Mismatch
checks examine activated and requested roots before a final interpreter whose root
was not already inspected. Uninspectable roots still count as seen. Lexical root
normalization preserves distinct symlink aliases here; PATH inventory separately
deduplicates resolved aliases and filters uv overlays, following source policy.

Source validation covers 832 note cases: four process-env values × two user-env
states × two embedded-env states × two managed states × thirteen version strings ×
two binary names. Unicode, signed and malformed versions include Python int()
whitespace boundaries. Native identity omits HCLI's own-Python-venv branch. The
expected compact JSON vector has SHA-256
`93d534110426832b3bc9831df8a4eb3c246f8008f869ccc8df1e51c5f79232e1`.

The mismatch oracle compares 144 cases: four interpreter/configuration profiles ×
three activated-root choices × three requested-root choices × four final-executable
choices. Rust executes owned shell interpreters; source version subprocesses are
replaced with corresponding observations while source path/configuration helpers
read the same owned files. No Python process edits files.

Thirty-two complete text reports compare actual source models/renderers with Rich
at width 10,000 and no color. They cover every section, empty/error states, null
probe values, note order and single/multiple-version warnings. The expected vector
has SHA-256
`7a54f5b3acf37ceb05e8f4aa5772840460927c877900c647411b3bdf4cd8631a`.
Both note/render vectors are pinned in default tests. Narrow-terminal wrapping,
color/highlighting and arbitrary Rich markup are outside this comparison.

Six CLI tests cover sixteen invocations: two overrides, embedded-venv inspection,
three mismatch configurations, unavailable versions and absent installation,
four PATH/activation states and four installation-version sources. They assert
probe counts, version calls, nulls, mismatch/configuration precedence, candidate
ordering and version provenance. Known-installation versions use SDK/directory
metadata; selected-version reports retain override/registry/SDK/binary/directory
provenance. The shared version helper preserves existing instance-version order.
Whole discovery/metadata and collector-exception equivalence remain open.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::explain
cargo test --offline --locked --test python_explain
```

Bounded findings: **medium impact**—an override can identify a virtualenv without
establishing that IDA activated it. **Medium impact**—an empty mismatch list does
not prove compatibility when version observations failed. **Medium impact**—an
interpreter's version can differ from its venv configuration and takes precedence.
**Low impact**—the source labels a resolved user venv as coming from PATH even when
VIRTUAL_ENV supplied it directly.

Interpreter selection has separate orchestration and shared layout helpers.
Creation and general derivation use the same ordered candidate generator, while
diagnostics and derivation share lexical normalization. Prefix candidates are
absolute paths; selecting sys.executable or a requested interpreter preserves its
input spelling. The outer IDAPYTHON override requires a file; derivation candidates
use exists(), as upstream does. Empty overrides remain unset because upstream ENV
applies _env_optional first.

The source corpus compares 11,520 derivations: four filesystem profiles × six
prefix/base-prefix pairs × eight executable observations × six requested-executable
values × five virtualenv values × two frozen states. Profiles include missing
prefix candidates, missing versioned names and directory candidates. The source
performs actual filesystem checks against Rust-owned fixtures without executing
candidate files. After replacing each temporary root with /fixture/N, the expected
vector has POSIX SHA-256
`9136a3481267e720dd39e0dcdf780d2848ecc633edc509db8171ec003c50be53`.
Default Unix tests pin that vector. Windows code is compiled separately; no native
Windows digest or runtime match is claimed.

Four read-only runtime cases execute the native and upstream probe payloads with
frozen true/false and sys.executable null/present, comparing common fields and
version. These concern the standalone inspection payload. IDA acquisition now
uses the exact upstream payload under A38; its separate adapter retains frozen
and nullable executable observations. Complete Pydantic coercion remains open.

Four CLI tests cover 23 invocations: seven derivation scenarios, twelve combinations
of override/variable states, two source-diagnostic failures and two cache scenarios.
The cache fixture runs explain-environment, which can request the IDA probe repeatedly:
success invokes idat once; a first failure invokes it again and retains the later
successful observation. This checks cache lifecycle without certifying the rest
of explain-environment's report; A39 covers that report separately. Creation's thirteen CLI tests and doctor's seven
tests also pass after adopting the shared layout helper.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::resolution
cargo test --offline --locked --test python_resolution
```

Bounded findings: **medium impact**—a Python-like executable name alone does not
validate sys.executable; matching virtualenv evidence or an explicit requested
executable is required before that fallback. **Medium impact**—derivation can
select an existing directory and leave execution to fail later. **Low impact**—the
source normalizes str(None) during virtualenv matching, so a VIRTUAL_ENV spelling
that normalizes to cwd/None can match an executable with no virtualenv root. The
corpus preserves this behavior. **Medium impact**—a failed cached probe must remain
retryable when a later operation in the same command needs it.

IDA probe acquisition uses separate orchestration, batch/log and startup-file
modules. The payload is compared directly with GET_PYTHON_INFO_PY, ignoring only
trailing whitespace. The adapter reads source version_major/version_minor fields;
internal consumers continue to receive a major.minor string. Unobserved pip/script
fields were removed from the shared probe record. The adapter currently accepts
signed 64-bit JSON integers and strict booleans/strings, while upstream Pydantic
supports additional coercions and arbitrary-precision integers. This remains an
explicit model-validation gap, along with caller exception-class mapping. Python
JSON extensions, lone-surrogate strings, recursion boundaries and exact malformed
JSON diagnostics are not established by the log-framing corpus.

Existing IDAUSR directories receive an initial interactive startup. Missing logs
or logs without a result permit one isolated retry. JSON decoding and model
validation failures do not trigger that retry. The isolated copy includes Python
configuration, registry/startup state and top-level *.hexlic files, including dot
names. It omits plugins and unrelated content. A source file masquerading as
IDAUSR is left in place and passed through for one attempt, as is an absent path.
Temporary script, log and isolated startup directories are removed after use.

The batch command uses the source argv, inherited cwd/stdin and no fixed timeout.
It removes VIRTUAL_ENV, PYTHONHOME, PYTHONPATH and PATH, then restores the resolved
user virtualenv when available. IDADIR, PYTHONUTF8, IDAPYTHON_VENV_EXECUTABLE and
other startup variables retain their inherited values. The native user-venv
resolver has the A33 limitation that a Rust executable has no Python sys.prefix
environment to exclude. Linux's IDA 9.2/path-with-spaces warning is retained.

Log validation compares 668 source cases: 660 combinations of eleven line
separators, six prefix lengths, five result shapes and two trailing separators,
plus eight byte/framing cases. Tests cover invalid UTF-8, BOM/NUL, the first
line-start marker, invalid JSON and the final twenty-line failure tail. The expected
compact JSON vector has SHA-256
`dfe56f10f6f790598bcd210f6ff72a351e86481e46f46b0443d768b25e28a406`.
One additional filesystem corpus compares selected copy names with the actual
source helper. Python source writes and subprocess calls are replaced in memory;
Rust creates the files and checks copied bytes and modification times.

Three CLI tests cover 29 invocations: three startup/fallback scenarios, 24
IDAUSR/log/status combinations and two stdin/environment scenarios. Fixtures
snapshot argv, payload, cwd, environment and startup files. They verify copied
license/configuration contents, preserved source files, first-attempt interactive
state, inherited fallback state, stdout exclusion, nonzero-status log success and
cleanup. Creation, doctor, guards and resolution regressions use source-shaped
probe documents and write their results into the requested log file.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::probe
cargo test --offline --locked --test python_probe
```

Bounded findings: **high impact**—the first probe uses the user's real startup
context, including plugins; isolated fallback can therefore observe a different
environment. These tests use owned shell programs and do not certify real plugin
startup effects. **Medium impact**—a usable log result takes precedence over a
nonzero child status. **Medium impact**—the source has no fixed batch deadline;
output capture and log reading have no added byte limit. **Medium impact**—license
and Python-selection files are necessary parts of the isolated startup context.

Script lookup and execution use separate orchestration, probe-result and subprocess
modules. The two embedded interpreter payloads retain the upstream implementation:
the first matching distribution supplies metadata and RECORD candidates, followed
by the interpreter directory, the sysconfig scripts directory and the user scripts
directory. RECORD candidates use realpaths; directory discovery preserves the
interpreter's virtualenv symlink. Entry-point fallback runs only for run-script;
find-script still requires a wrapper. Wrapper dispatch uses os.access-style
executable permission rather than a filename extension.

Probe capture has a 60 s deadline, inherited stdin, explicit UTF-8 replacement and
universal-newline decoding. A failed status takes precedence over result text;
the first line beginning exactly with __hcli__: supplies JSON. Native result and
diagnostic adapters compare 198 framing/status cases and 48 metadata/directory
cases with the actual source helpers. The expected compact JSON vectors have
SHA-256 digests, respectively:
`874d2222afa6dd0bae53bef03b2fdd05523ef889c14d07fa4997f2f5a49cb3a9` and
`946fa7f9a3fc01b63ae021af47249de6c9f9f07b7f7bdeaeb1eeb362c694235a`.
Default tests pin those digests. The optional source check also compares both
embedded interpreter payloads with upstream, ignoring only trailing whitespace.

Six CLI tests cover 40 invocations with the oracle runtime configured. Thirty-two
fixture processes cover permission/extension combinations, fallback versus lookup,
failed and malformed probes, environment/stdin inheritance and Unix signal status.
Environment coverage includes exec, metadata fallback and a wrapper outside the
interpreter directory, each with and without a virtualenv and with PATH unset,
empty or containing empty components. Eight real-interpreter invocations use
temporary distribution metadata and modules to exercise missing RECORD entries,
RECORD symlinks, non-executable Python wrappers, user scripts and metadata fallback.
Rust creates every fixture file; Python runs with bytecode writes disabled and
does not edit files. The source oracle loads only selected AST definitions and
records probe subprocess calls in memory.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::scripts
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test python_scripts
```

Bounded findings: **medium impact**—missing wrappers do not imply missing entry
points; execution can succeed while find-script fails. **Medium impact**—wrapper
permissions select direct execution versus interpretation, independently of .py
suffixes. **Medium impact**—child environment construction uses the selected
interpreter even when its wrapper lives elsewhere. **Low impact**—inherited
PYTHONUTF8 and PATH empty components retain their spelling. Unix child signal
return codes use Python's negative-number convention, then the CLI's exit-code
conversion; process-group KeyboardInterrupt behavior remains outside this corpus.

Environment guards now share doctor observations and policy through a small guard
module. An explicit probe mode distinguishes use of the resolution's existing IDA
probe from permission to obtain an additional one. The collector borrows the
resolved interpreter, allowing installation and execution to keep using the same
selection. Installation checks run after combined metadata parsing and before
wheelhouse extraction, editable registration preparation or pip execution.

Ten read-only source cases compare complete warning blocks and exception bodies,
including empty findings, both severities, mixed ordering, literal Rich-looking
path text, Unicode and two binary names. Unstyled source rendering uses 10,000
columns. The compact expected JSON vector has SHA-256
`6655aed1f88084f185df0d282ef54951cbea93e747ee69cdde79b5c52dbf29be`,
pinned by the default unit test.

Six CLI tests run 36 isolated processes: six exec/run-script/find-script variants,
one configured-interpreter advisory check, two parent-versus-child option cases,
twelve original installs plus twelve upgrade attempts, one malformed-neighbor
case and two explicit doctor invocations. The upgrade cases cover a healthy venv,
an override warning, base Python, missing pip, version mismatch and uv overlay,
each with the skip flag enabled/disabled. Rejected upgrades retain the original
manifest and sentinel bytes and never invoke pip. Warning-only commands preserve
child status 7; find-script prints its path with status 0. The advisory path does
not run the available fake IDA executable. Installed metadata errors occur before
any interpreter observation.

Migration retains its existing source-report oracle and now asserts exactly two
pip installation invocations for two plugins, without a dry-run. The source's
`install_single_plugin_dependencies` directly calls pip installation; it neither
validates the currently selected global environment nor repeats package resolution.
The newly created target has already passed creation's version/pip validation.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy environment_guard_messages
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test create_environment
cargo test --offline --locked --test python_environment_guard
```

Bounded findings: **high impact**—diagnostic error findings block dependency
installation but must not prevent execution commands used to inspect or repair
that interpreter. **Medium impact**—skipping diagnostics still requires an
importable pip before installation. **Medium impact**—retained-plugin metadata
failure precedes interpreter probes, preventing an unrelated environment finding
from replacing the input error. **Low impact**—the warning-only path can print an
`Error:` header while running the requested child; this wording comes from the
shared upstream formatter and does not determine the execution command's status.

Doctor separates observation from interpretation. State retains unknown version,
pip and embedded-version values, while filesystem-dependent variable selection
and Homebrew classification are collected before policy evaluation. The findings
function receives the binary name explicitly and performs no I/O. Setup
classification consumes the already-computed findings instead of recomputing them.
Creation and doctor share independent 10 s version and pip subprocess checks.

The policy oracle compares 4,106 states with the pinned
`check_python_environment` and `identify_setup_pattern`. It covers 4,096 boolean
combinations with correlated unknown/version/pip variations, nine platform/path
cases and an explicit healthy state. This is not the Cartesian product of every
field. All twelve pattern IDs occur. Complete ordered findings, severities,
summaries, details, hints and pattern text are compared. The oracle supplies the
filesystem-dependent variable-selection and Homebrew predicates from each case.
After replacing backslashes with slashes in expected JSON strings, the compact
expected vector hashes to
`c159a75d03ae7457419acaf44dd232eb48c45e082b57ba27d275ef1584f85686`.
The default suite pins this value; source comparisons use unmodified values.

A separate seventeen-case oracle reads owned files and compares interpreter-layout
recognition, variable-to-venv matching, pyvenv.cfg parsing and Homebrew prefixes.
Fixtures cover file versus directory pyvenv.cfg, Python-name casing, unrecognized
layouts, duplicate/case-insensitive keys, CR newlines and literal backslashes.
Python reads these files; Rust creates them. Neither oracle edits project or user
files through Python.

Seven native CLI tests exercise 31 processes. They check override probe bypass,
Python 3.9 without an invented minimum-version warning, independent pip failure,
embedded-version mismatch, startup-script text detection without execution,
missing versus unrunnable interpreters, unresolved Python, managed-base pip
suppression, three uv-overlay signals, shell activation and conda metadata.
Eight report renderings compare exact unstyled text at a source width of 10,000
columns, including successful/no-finding output, unresolved output, context notes
and a custom binary name. These rendering comparisons supply the native JSON
report to the source renderer; they certify presentation of those fields, not the
entire upstream collection pipeline.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::environment
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test python_doctor
```

Bounded findings: **high impact**—an explicit HCLI interpreter override bypasses
IDA probing; a doctor invocation must not infer an embedded-version mismatch from
an unrelated IDA installation. **Medium impact**—the source's PEP 668 decision
depends on the probe carried by resolution, not an independent marker observation
from the selected interpreter. Explicit variable resolution can therefore report
externally_managed=false even when a later IDA probe has a marker. **Medium
impact**—startup activation is a substring observation, including comments; it is
not evidence that the script activates a venv when executed. **Low impact**—source
Homebrew detection accepts a textual prefix such as `/opt/homebrewish`, and its
venv-root helper accepts a directory named pyvenv.cfg through exists(). These
quirks are retained. The Windows context note still mentions setx although the
configuration writer uses PowerShell's Environment API; this is an upstream text
inconsistency, not a change to the writer.

Platform configuration separates detection, per-OS plans, file semantics, execution
and reporting. Each step has one typed action: a file path/content pair or a program
with arguments. The source dataclass's optional fields are reconstructed only at
the test oracle boundary. LaunchAgent XML and Windows manual text are separate
templates; command arguments remain distinct from display text.

Three platform tests compare 504 plans (3 systems × 7 shell values × 2 systemd
states × 4 sessions × 3 values), 27 session-variable combinations and 152 file
updates (2 step kinds × 4 contents × 19 initial states). Plans compare all step
fields, warnings, manual instructions and logout requirements. File cases include
duplicate/stale assignments, leading whitespace, CRLF/CR, Python splitlines
separators, invalid UTF-8 and existing exact content. Skips preserve original bytes.
The source oracle uses in-memory file objects; Python does not edit fixture files.

Default tests pin SHA-256 over compact JSON expected-result vectors. Canonicalization
replaces backslashes with slashes in strings and removes CR before LF in file-byte
arrays for host-independent hashes. The optional source oracle still compares
unmodified full values before any such normalization:

- Plans: `9b2d7a8899648b748934458b4260276eb73d54488ed8373305f6957ccb0082c6`.
- Sessions: `bbbd76b43e0a411b55a66997d1e790e21ef5bd65a70e094715027004f5abaaff`.
- Files: `7b526a38c5037a9b99cb729e1f53a6cb501e2126d2accc131334ee414e6d692c`.

A fourth test compares eight configuration-report outcomes with the actual source
command function, including no steps, all skipped, partial success and failure.
Five macOS CLI tests cover nine invocations: first/repeated configuration, command
failure followed by a profile update, two file-failure positions, three unknown
shell values and a PTY decline after displaying the entire plan. Owned homes and a
fake launchctl contain every persistent effect; no real session is changed.

```sh
HY_TEST_VENV_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy ida::python::
cargo test --offline --locked --test python_configuration
```

Bounded findings: **high impact**—the pinned fish prefix helper returns `set ` for
ordinary exports and can replace unrelated assignments. **Medium impact**—shell
and XML templates interpolate literal values; matching their bytes does not prove
that every path containing quotes, newlines or XML metacharacters yields a valid
configuration. Such values are plan-only fixtures. **Medium impact**—command
failures continue execution while file exceptions abort, preserving prior effects;
neither implementation rolls back earlier changes. **Low impact**—successful
all-skipped or empty plans report configured=true without a mechanism name.

Upstream sets its process environment after all steps finish if any succeeded.
Hy supplies that value explicitly to verification's child process, avoiding an
unsafe process-wide environment mutation inside Tokio. Arbitrary subsequent
same-process reads remain a parity gap. Verification checks current inheritance
(or the Windows user setting), not persistence across future login sessions.
Exact OS exception text, non-UTF-8 subprocess output, negative signal return codes
and native Windows/Linux persistence remain outside the passing corpus under A32.

API error rendering now uses `util/python_repr`, separating container traversal,
number notation, string quoting and the pinned Unicode printable table. The table
contains 712 sorted inclusive ranges from CPython 3.13.15's Unicode 15.1.0
`str.isprintable()`. It is data, independent of the Rust toolchain's Unicode
version. Grouping consecutive printable code points over 0..=0x10ffff reproduces
it. The full classification has 149,625 printable code points and SHA-256
`bf0b550942f03d97e5623f688d8398cc2222ecfa58bfdef567d72d4bb994e4d9`
over one 0/1 byte per code point. Single-character repr output covers all 1,112,064
Unicode scalar values; UTF-8 output followed by a NUL byte per scalar hashes to
`6c6a8f1cd7e21042c3e29990d5fccc93c3b5e8c969136ad1e91f664ee57a0ca3`.

The numeric formatter parses JSON float lexemes as binary64, uses Ryu's shortest
ties-to-even digits, and applies Python's fixed/scientific threshold and exponent
sign/padding. Ryu 1.0.23 was already locked transitively and is now a direct
dependency; no new package version was introduced. A 34,658-case CPython oracle
covers exponent boundaries, zero signs, overflow/underflow and deterministic
binary64 bit patterns. A further 12,993 cases cover containers, arbitrary integers,
quoting, escapes and Unicode strings. SHA-256 over UTF-8 outputs joined with NUL
bytes (no final NUL) is respectively
`c0e0ee243a6fc3820a4635ecc6af0b8b9907a6e40cdad53897bb32f937c120d1` and
`06802c207bf3d6e5d6bfb72e805e7d0cb06de8bda5675d6b27e4fa5294c440e6`.

An additional 203-case source oracle executes the pinned
`APIClient._handle_response` method and compares exception class and `str(error)`
at seven HTTP statuses. Its 29 documents include every standard JSON value kind,
duplicate object keys, missing/malformed messages, float overflow, integer limits
and non-object responses. The serialized expected result array hashes to
`ef33b340348471e1cb406df7662de61b1851ce56b680156f2440b220a558a177`.
Two CLI tests cover 24 JSON-method failures and eight signed PUT failures, including
the absence of confirmation requests after unsuccessful transfers.

Primary sources are the pinned `src/hcli/lib/api/common.py::_handle_response`,
CPython 3.13.15 `str`, `repr`, `json.loads` and Unicode 15.1.0 runtime observations,
and the locked Ryu 1.0.23 `src/d2s.rs` ties-to-even branch. Python oracle processes
read source and exchange JSON through pipes; source/table edits use apply_patch.

```sh
HY_TEST_JSON_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy python_repr
HY_TEST_JSON_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --bin hy api::json
cargo test --offline --locked --test api_errors
```

Bounded findings: **medium impact**—discarding a non-string message loses the
server's supplied diagnostic. **Low impact**—Rust's Debug decimal tie rounding
differs from Python: the tested value represented by -2.0156234722508763e14 displays
as -201562347225087.63 with Debug and -201562347225087.62 with Python/Ryu.
**Low impact**—using the Rust toolchain's newer Unicode classification would alter
escaping for characters unassigned in the upstream Unicode database.

API redirect validation uses six integration tests spanning thirty-six scenarios.
Twenty-two cases combine eleven statuses (200, 201, 300, 301, 302, 303, 304, 307,
308, 400, 500) with present/absent Location headers. They execute the upstream
APIClient.put_file body against HTTPX 0.28.1 using a custom AsyncBaseTransport that
consumes request.stream directly. HTTPX MockTransport calls Request.aread(), which
replaces consumed generator streams with replayable ByteStream objects; it cannot
prove real upload replay behavior. The corrected oracle verifies success and
completed method/path events, including StreamConsumed for replay redirects.

Additional native cases cover chains of 19, 20 and 21 redirects, JSON GET/POST/
DELETE redirect responses, one truncated successful upload response, a cross-origin
download retaining x-api-key, and six terminal cache HEAD statuses. A truncated
upload response fails before confirmation. Cache reuse accepts status below 400
when the content length matches, retaining the existing native checksum check.
All fixtures use owned files and loopback servers. The source oracle's stream
iteration does not certify whether partial redirected PUT headers reach a server
before HTTPX raises StreamConsumed; the native implementation fails before sending
that second request.

Primary sources are the pinned `src/hcli/lib/api/common.py`, HTTPX 0.28.1
`_client.py::{_send_handling_redirects,_redirect_method,_redirect_headers,_redirect_stream}`,
`_content.py::AsyncIteratorByteStream`, `_models.py::Request.aread` and
`_transports/mock.py::MockTransport`. Reqwest 0.12.28
`async_impl/client.rs::{execute_request,PendingRequest::poll}` shows that the read
timeout starts before the response headers arrive, including outstanding upload
time. The native configuration therefore removes the overall response deadline
but does not yet match HTTPX's unlimited upload-write phase.

```sh
HY_TEST_UPLOAD_ORACLE_PYTHON=/tmp/hy-pydantic-locked/bin/python \
  cargo test --offline --locked --test api_redirects
```

Bounded findings: **high impact**—implicitly stopping at a streamed redirect could
confirm an upload that upstream rejects. **High impact**—a buffering mock transport
can incorrectly certify replayability; the oracle now preserves the original
stream. **Medium impact**—successful response headers alone do not prove upload
completion when the response body is truncated. **Medium impact**—reqwest read
configuration does not by itself establish separate read/write inactivity limits.
Configuration tests verify memory rollback and migration without resurrecting
removed instances. No live authentication or account mutation is performed.

Authentication runtime fixtures cover GoTrue request ordering and headers, valid
and rejected user responses, opaque/empty tokens, environment-key precedence,
forced credential/type constraints and anonymous optional repository requests.
Terminal tests attach the CLI to a temporary controlling PTY and drive real
dialoguer prompts for OTP and logout. They check that failed user verification
does not write credentials, cancellation preserves configuration bytes, forced
OTP sign-out clears legacy refresh data, and failed logout persistence retains
the original credential/session file. The PTY and HTTP fixtures use bounded waits
and terminate their child processes/listeners on failure; no browser is launched
by these tests. Separate loopback socket tests cover OAuth callback transport.

OAuth transport tests send fragmented HTTP headers and bodies, oversized declared
and chunked bodies, invalid JSON and token fields, and concurrent incomplete
requests. They verify complete responses and listener/connection shutdown on the
overall deadline. The authorization URL test round-trips redirect query parameters
and forced-account selection. Browser JavaScript execution requires this manual
fixture, which prints a loopback URL containing only synthetic tokens:

```sh
cargo test --bin hy auth::oauth::tests::browser_callback_round_trip -- --ignored --nocapture
```

The manual attempt was terminated after browser automation could not obtain a
usable browser window. No browser round-trip is claimed, and no test listener was
left running.

Editable tests use an interpreter fixture for purelib discovery and pip calls.
One import test requires `python3` on PATH and runs it with `-I -B -S`: the test
explicitly adds its temporary site directory, imports only its fixture module,
and checks that source changes appear without reinstallation. Python does not
write fixture files or bytecode. Tests default to a nonexistent IDA interpreter
override, preventing cleanup discovery from launching the developer's IDA.
Failure fixtures cover dependency resolution, registration staging/publication,
configuration cleanup, broken links, plain files and source preservation.

Directory and inventory regressions cover native distribution versus editable
rules, packaging filters, file/directory symlinks, dangling source links, modern
versus minimal descriptors, missing files, exact directory-name matching,
case-only publication, repeated status requests and dependency exclusion of
broken neighbors. They also verify explicit removal of older filenames while
rejecting path traversal. No installed plugin code is executed during discovery.

Repository search parses each repository once per command. For P plugin identities,
I installed plugins, and V versions per identity, local report collection takes
O(P × (I + V log V)) time, excluding I/O, and O(PV + I) retained metadata space.
Executable architecture inspection reads a fixed-size header and, for PE, performs
one seek and one additional bounded read: O(1) time and space relative to file size.
Archive referenced-file validation builds a hash-set inventory in O(N) expected time
and O(N) space for N ZIP entries. For P descriptors with D total declared platform
entries, validation performs O(P + D) expected lookups, excluding metadata parsing
and path lengths. Repeated declarations are included in D. Update download staging
streams B bytes in O(B) time.
Inline dependency extraction scans an S-byte script and parses its metadata in
O(S) time with O(S) temporary space. IDA range expansion checks a fixed catalogue
of 63 versions; declared-version membership is linear in the declaration length.
Protocol response accumulation uses at most 1 MiB of payload plus one 4 KiB read
buffer; incremental JSON parsing can revisit the accumulated prefix.
IPC discovery retains N candidates in O(N) space and probes each once. Named lookup
queries at most N candidates in source order and returns immediately on a match;
relative lookup queries all candidates. Unix filesystem latency is additional.
Windows enumeration uses 4096 DWORD slots (16,384 bytes), matching the upstream cap.
An IPC timeout applies separately to each operation, not to the entire inventory;
startup can therefore overshoot its deadline while the current discovery cycle runs.
OAuth accepts at most 16 concurrent connections, with a 2-second deadline per
connection and a 120-second deadline for the login flow. Hyper's connection buffer
is limited to 16 KiB and token bodies to 64 KiB (65,536 bytes); JSON decoding and
transport processing take O(B) time/space per accepted B-byte body. These bounds
describe protocol buffers, not the entire runtime's heap allocation.
Command inventory collects C visible leaves and sorts them in O(C log C) time
with O(C) retained rows. Root help reads local JSON and relevant SDK/binary bytes;
these version readers currently allocate memory proportional to each file read.
Help status is constructed for help requests, not ordinary command dispatch.
Shared-file filtering stores B bytes of lowercased labels and O(N) group, selected
occurrence and visible indices. Construction performs O(N²) Asset comparisons;
comparison cost includes field bytes and recursive metadata numeric conversion.
An edit rescans labels for a query of length Q: conservatively O(BQ) time.
Toggle-all, inversion and original-order submission take O(N²) time. Navigation
takes O(1); a toggle and each rendered selection lookup take O(N). Rendering reads
the current terminal page; query/selection state persists across pages. Asset
integer normalization scans D bytes with O(D) retained text and a conservative
O(D²) arbitrary-precision decimal conversion bound. Accepted significant integer
digits are bounded at 4,300; input strings and aggregate responses have no new
independent byte quota. Metadata recursion has no independent depth bound beyond
the JSON decoder. These are compatibility contracts, not resource quotas.
Standalone confirmation scans the total submitted input bytes once, retaining one
line at a time. Retries have no added deadline or input-byte quota. Native canonical
terminal editing remains owned by the operating system. Report formatting retains
the displayed field strings; download transport and path resolution retain their
separately documented complexity and filesystem-race limitations.
The visibility/action selector retains one cursor and borrowed choices. Navigation
is O(1); each redraw scans the choice text and emits one row per choice. The current
call sites contain three and two choices respectively. Terminal-mode restoration
retains only the prior enabled state and runs on every normal or error return.
MCP discovery probes a fixed set of five command names across PATH. Listing scans
take O(B) time for B decoded bytes; JSON traversal adds O(D) call-stack depth for
tree depth D after serde parsing. Captured output and decoded text require O(B)
memory; no capture quota or subprocess timeout is imposed. Scoped setup executes
at most four agent commands after IDA plugin installation. The fixed-name fold
adapter retains at most one line/value at a time in addition to the parsed listing.
Update discovery scans R releases in O(R) comparisons, excluding tag parsing and
specification matching. It retains one response page (requested size: 100) and the
best candidate; the current implementation does not bound individual JSON response
bytes. The background worker adds constant synchronization state and uses a
condition variable for completion. Successful command completion waits at most the
configured 2 s for that signal, excluding scheduler delay; it does not wait for the
network worker to terminate. Cache timestamps use microsecond precision and a strict
86,400 s age threshold, with future timestamps treated as recent.
Extension ownership snapshots retain command/parameter representations before
registration and compare them afterward. For N commands with S total attribute
representation bytes, snapshot work is O(N + S), excluding arbitrary repr/import
and registration code. Recursive dictionary merging can revisit ancestor paths;
the complete bridge therefore has no general linear-time guarantee. Its serialized
report is limited to 1 MiB (1,048,576 bytes). Interpreter memory, import/registration
time, and extension execution are not bounded by that protocol limit.
Instance listing reads metadata once per row and sorts N rows in O(N log N)
comparisons with O(N) retained rows, excluding metadata bytes. Default selection
uses O(N) comparisons and retains one candidate. Discovery retains P paths and a
hash-set of canonical identities in O(P) space; filesystem and registry latency
are additional costs. Spotlight has a 10-second subprocess deadline and buffers
its output; no application binary is executed during discovery.
Protocol setup retains N discovered registrations and selects their default in
O(N) comparisons, excluding version-file bytes and filesystem latency. Source
listing takes O(S) filesystem existence probes in registration order. Source-name
validation scans B input bytes in O(B) time and O(1) auxiliary space. Protocol
launcher quoting takes O(B) time/space for B path bytes; native compiler, shell,
Launch Services and desktop-tool subprocesses have no added timeout.
Ordinary URI parsing takes O(U) time/space for U input characters. Filename-pattern
construction takes O(P) time/space for P pattern characters; a final-bracket guard
avoids repeated scans of unclosed-class suffixes. Matching N filename characters
takes O(NP) time and O(P) state, plus case-folding storage on Windows. Recursive
lookup retains pending directory paths, skips directory symlinks and follows file
symlinks. Its I/O count scales with visited entries; filesystem latency is unbounded.
Download matching without backtracking delegates to the regex engine; lookaround
and backreference patterns can require exponential backtracking work. The engine
limits backtracking to 1,000,000 operations, which is not a wall-clock deadline.
Setting-pattern translation scans P pattern characters in O(P) time and space.
Compilation and matching retain the regex engine's costs and backtracking limit.
Prefix matching checks the returned match's start position; the engine can search
later positions before returning, unlike Python's anchored `re.match`, so runtime
and backtracking-limit equivalence are not claimed. Interactive configuration
prepares S questions and scans C total choices in O(S + C) work, excluding string
comparison lengths; retained answers require space proportional to their bytes.
KE query iteration and raw-field filtering take O(U) time/space for U URI characters,
excluding the content URL parser and host canonicalization;
percent decoding retains storage proportional to decoded query bytes. The last url
value is selected in one pass. Optional navigation avoids a separate boolean flag
and leaves the database startup/analysis policy unchanged.
Unix cache-path resolution uses an explicit component stack and caches completed
link targets. For C expanded components and maximum intermediate path length P,
path copying/hashing takes expected O(CP) time and at most O(CP) retained bytes,
excluding filesystem latency. It probes metadata per visited component and reads
each distinct expanded link once. Windows tries up to D ancestor prefixes for depth D;
canonicalization cost belongs to the OS. Both implementations inspect paths without
creating them, and neither makes the subsequent filesystem operations race-free.
Datetime parsing scans N input bytes in O(N) time with O(1) parser state. Report
callers additionally allocate O(N) bytes for upstream's Z replacement. Fractions
truncate to microseconds (10⁻⁶ s); timezone offsets retain that precision and must
have magnitude below 86,400 s. Relative-format arithmetic uses integer day and
second remainders, with each day defined as 86,400 s, rather than calendar months.
Lint scans E archive names once for each of P validated plugins: O(PE) name checks,
excluding string lengths and the shared descriptor/file validation work. It retains
O(E) borrowed names, parsed metadata and the ZIP inventory. Directory README scanning
retains filenames proportional to their total bytes and performs one file-type probe
per entry. Remote archives remain buffered; lint introduces no archive-size limit.
KE retention collects N paths and sorts them in O(N log N) comparisons with O(N)
retained paths, excluding path bytes and filesystem latency. Each cleanup phase
performs O(N) file-type/metadata or directory probes. Before traversal, multiplying
D integer limbs by the fixed 86,400 s/day factor takes O(D) time and O(D) temporary
storage. The integer product is rounded to binary64 once before subtraction from
the current time in seconds; non-finite conversion fails before traversal. Dialog command construction
is O(T) in message length; native confirmation/error waits have no added deadline.
Progress dismissal waits at most 2 s after requesting termination.
KE transfers hash B decoded payload bytes in O(B) time. HTTP decoding processes
the compressed input plus each layer's output; its work scales with their combined
size, not just B. A 64 KiB scratch buffer feeds per-chunk output vectors. Peak retained
data includes adjacent layer buffers and codec state; intermediate expansion and an
unlimited content setting do not provide a fixed memory bound. Once the 16 MiB decoded
metadata limit is enforced, JSON validation reads B bytes, decodes character encoding,
adapts tokens and checks syntax in O(B) time. Its storage is O(B), plus O(D) state for
nesting depth D, without constructing a JSON value tree. Normalized validation bytes
are discarded; the decoded payload bytes are published unchanged. The payload limit
does not bound total heap allocation to 16 MiB. Disk-space checks occur per decoded
chunk in Hy, more frequently than upstream's 32 MiB interval.
KE address validation uses O(A²) equality comparisons and O(A) retained addresses for
A resolver results, matching upstream's ordered list deduplication. It constructs up
to A HTTP clients, tries at most A connections per hop and processes at most six
responses across five redirects. Connect and read inactivity limits are 300 s each;
there is no total-transfer deadline. DNS lookup and filesystem work have no new
deadline. IP classification itself uses fixed-size integer operations and O(1) space.
Proxy environment collection scans E total variable bytes twice. It sorts P mounts
in O(P log P) comparisons and searches them in priority order for each request; host
comparison cost scales with the compared strings. HTTP proxy requests own one HTTP
connection driver, or up to two across CONNECT; bodies stream through Hyper into the
existing response decoder. TCP/TLS connection deadlines and read/write inactivity
limits are 300 s. Connections are not pooled, and these bounds do not specify total
TLS/HTTP heap allocation or an overall transfer deadline.
OS discovery occurs only for an empty environment map. macOS reads five protocol
groups from one snapshot; retained URL text scales with the configured host lengths.
Windows parsing takes O(S) time and O(S) storage for S ProxyServer bytes, including
the optional expansion of a single proxy into three protocol entries.
CA-file parsing scans B PEM bytes and retains O(B) trust-anchor data, with no new
independent file-size quota. The session shares each TLS context through Arc;
reqwest clients clone the origin ClientConfig while sharing its internal verifier.
The proxy root store adds the bundled roots to any configured file roots.
Cookie selection scans N stored cookies, then stably sorts M matching cookies in
O(M log M) comparisons. Domain/path comparisons additionally scale with their string
lengths. Storage retains cookie text and domain/path keys; empty parent maps preserve
Python insertion order. Response parsing temporarily retains nonexpired cookies until
the batch completes. No independent cookie count or aggregate-byte quota is imposed.
Cookie date expressions are compiled once. Parsing H header bytes uses O(H) temporary
attribute storage; fixed regex patterns and numeric conversion scan their input.
Calendar conversion uses fixed-size integer arithmetic in seconds, with 86,400 s/day
and 3,600 s/hour. Netscape date parsing completes before that format's jar mutations;
legacy Set-Cookie2 construction runs first and can already have deleted stored cookies.
Integer syntax scanning takes O(D log R) work for D characters and R=68 decimal-zero
ranges, with at most 4300 normalized digits. Arbitrary-precision decimal conversion
retains O(D) space and has a conservative O(D²) arithmetic bound. Normalization retains
integers for the response before construction; a numeric limit does not cap total jar
or header memory. Stored expiry comparison uses arbitrary-precision integer values.
Legacy header tokenization retains O(H) text for H input bytes. Malformed quoted values
can cause repeated scans of remaining suffixes, giving a conservative O(H²) work bound;
no added header-token count or cookie-storage quota is claimed.
Cache writing and hashing take O(B) time for B decoded bytes; final download
responses are not buffered in full. Content-decoding work and intermediate storage
follow the layer-dependent bounds above. Existing cache checksum validation reads B
bytes before reuse. Metadata sidecars are small but published separately.
License discovery sorts L records in O(L log L) time with O(L) retained metadata;
filtering and display grouping are linear in L. Local license copying streams B
bytes in O(B) time, excluding filesystem metadata operations.
Asset upload hashing scans B file bytes in O(B) time with an 8 KiB buffer and
constant hash state; the subsequent PUT also streams. Request metadata memory
depends on the supplied permission/metadata fields, not the uploaded file size.
API JSON encoding detection examines at most four initial bytes. Decoding and
number scanning take O(B) time for B response bytes, followed by JSON parsing and
model construction. Valid UTF-8 borrows its text; UTF-16/32 conversion retains O(B)
temporary storage, including UTF-16 code units. KE's syntax-only normalization
retains a separate disposable byte vector. The 4,300-digit bound limits individual
integer tokens, not total input or aggregate value-tree memory.
API error rendering visits V JSON values and emits O(B) output bytes. Unicode
printability uses O(log R) comparisons per character for R=712 fixed ranges;
container traversal retains O(D) call-stack frames for depth D, plus output storage.
Number lexemes are scanned and float lexemes parsed before fixed-size binary64
formatting; Ryu and decimal-notation adjustment use bounded temporary state for
that fixed numeric domain. Integer output preserves the already-validated decimal
text. These bounds do not reproduce CPython's recursion-limit behavior.
Explicit API redirect handling sends at most 21 requests and retains one response
at a time. Header cloning scales with their bytes; URL parsing scales with target
text. Intermediate response bodies and final upload responses are fully buffered
before advancing, so peak memory includes the largest such body with no new byte
quota. Final download responses continue through the existing streaming cache
writer. These bounds do not establish a total-transfer deadline or full process
memory bound.
Global configuration commits clone and serialize C bytes of configuration in
O(C) time and temporary space. Credential insertion order is preserved through
JSON parsing and serialization; removing an entry shifts subsequent entries.
Directory copying takes O(B + sum(n_d log n_d)) time for B copied bytes and n_d
entries in each visited directory d, excluding filesystem latency and path lengths.
It retains sorted entries along the active recursion path. Canonical discovery
parses M total metadata bytes and sorts I valid records in O(M + I log I) time,
excluding referenced-file checks; retained records occupy O(M + I) space.
Environment creation scans at most three PATH command names and up to three
registered-prefix candidates on POSIX (two layout candidates on Windows). Each
version/pip probe has a 10 s deadline; creation and ensurepip each have a separate
600 s deadline. IDA probing has no fixed deadline under A38. Target inspection
checks only the first directory entry when deciding whether a non-venv directory
is empty. Plugin collection and dependency parsing scale with installed metadata
and dependency text; migration is sequential and retains one result per plugin.
These independent deadlines do not establish a whole-command timeout or reproduce
all upstream subprocess/environment behavior.
Platform plans contain at most three steps. Profile processing takes O(B) time and
O(B) memory for B input/output bytes, including lossy UTF-8 decoding, newline
normalization and retained line slices. File I/O has no added deadline. Each
configuration command has a separate 60 s deadline; verification has 5 s on POSIX
and 15 s on Windows. These are per-operation limits, not a whole-command deadline.
Doctor evaluates a fixed number of findings and twelve ordered patterns, with work
and output storage proportional to interpolated text. Filesystem observation reads
startup/configuration text in O(B) time and memory for B bytes, plus path resolution
and PATH traversal costs; no new file-size bound is imposed. Pip and version have
separate 10 s deadlines, optional IDA probing has no fixed deadline, and a recommended-venv
note may run another 10 s version probe. There is no whole-doctor deadline.
Guard formatting scans F findings and emits O(B) text, with O(B) retained output.
Advisory checks can spend 10 s each on pip and version; they do not add an IDA
probe to an already-resolved interpreter. Installation validation can add the
unbounded IDA probe and always runs a separate 10 s pip check after successful
or skipped diagnostics. Pip resolution and installation retain separate native
unbounded waits, matching the source helpers. Their stdout/stderr storage scales
with total emitted bytes B; UTF-8 replacement, stripping and message construction
take O(B) time and memory. Argument construction takes O(A) work/storage for A
option and requirement bytes, including an option clone when a wheelhouse is merged.
These bounds impose no process-output limit and do not certify signal/cancellation
behavior or real package-manager resource use under A35.
Script lookup's captured streams and decoding require O(B) memory/work for B
output bytes; JSON parsing and diagnostic construction scale with document/output
size. The interpreter scans distributions until its first matching entry point,
then that distribution's file list and the ordered candidate paths. Work depends
on metadata and filesystem latency; retained candidate paths scale with their
aggregate bytes. The 60 s deadline covers lookup only. Script execution has no
fixed deadline and attaches terminal streams without native output buffering.
These bounds do not certify process-group cancellation, metadata-size limits or
runtime resource use under A36.
Interpreter derivation generates at most six POSIX candidates or four Windows
candidates from two distinct nonempty prefixes. Each fixed selection stage scans
that list and performs bounded-count filesystem observations. Path normalization
and failure rendering scale with input/output path bytes, using proportional
temporary storage. A failed IDA probe is not retained; later callers may repeat its
unbounded operation. This provides no whole-command deadline or guarantee
against changes between filesystem observations under A37.
IDA batch acquisition captures O(B) stdout/stderr bytes and reads O(L) log bytes;
decoding, line framing and failure-tail construction use O(L) work/storage.
The isolated copy scans N top-level entries and streams C selected file bytes,
using O(N + C) work plus path/metadata costs and bounded per-file copy buffers.
Each acquisition runs at most two sequential batches, without a fixed deadline
or total output quota. These bounds exclude resources used by IDA and plugins.

Explain PATH collection scans P entries and retains their path bytes and resolved
roots. Known installations are sorted in O(I log I) comparisons. Mismatch checks
consider at most two virtualenv roots and one final interpreter; each version
subprocess has a 10 s deadline, and configuration fallback reads O(B) file bytes.
Source-order collection can repeat version observations. Text rendering emits
O(R) bytes with proportional retained output. Cached IDA acquisition has no fixed
deadline, so these limits do not bound whole-command time under A39.

Find-links conversion takes O(P + H) time and memory for P input path bytes and H
expanded home bytes, excluding account-service latency. It does not access the
referenced source directory. Reentrant Unix account lookup grows its buffer on
ERANGE and has no added service deadline. Bundle download argv construction takes
O(A) work/storage for A argument bytes; captured streams require O(B) memory and
failure decoding/rendering takes O(B) work for B output bytes. No fixed timeout or
output quota is added to the pip process. These bounds do not establish whole-command
resource limits or native process-group behavior under A40/A41.

Target grammar scans O(L) input characters, with O(L) retained text. Decimal
conversion uses arbitrary-precision arithmetic on at most 4,300 digits per component;
an O(D²) time upper bound and O(D) space cover D-digit parsing/comparison here.
Selection forms P × V expanded platform/version pairs and retains U unique target
IDs with expected hash-set lookup cost proportional to their byte lengths. Repeated
current-version entries each invoke the independent 10 s probe; this is not a
whole-selection deadline because interpreter/IDA resolution can add unbounded work.
Explicit target lists retain all input entries. These limits depend on A42.

Version probing captures O(B) bytes across stdout/stderr. Strict decoding, newline
translation and stripping take O(B) time and memory. Its 10 s per-process deadline
and lack of a byte quota do not bound a whole command that performs several probes
or resolves IDA. The Unix deadline test covers a process that replaces its shell
with sleep, not arbitrary descendants or inherited-pipe lifetimes, under A43.

Pip availability captures O(B) bytes without decoding; its 10 s timeout does not
impose a byte quota or a whole-command deadline. Bundle descriptor reading scans N
archive entries, reads M candidate metadata bytes and retains accepted records in
O(N + M) work/storage bounds, excluding ZIP decompression and model-validation costs.
Explicit requirement collection retains O(R) requirement bytes. Entry-point script
contents are not read during this collection. These bounds depend on A44/A45.

Bundle inspection traverses N outer entries and M inner entries, reading B selected
archive/descriptor bytes and retaining P accepted metadata bytes. It uses O(N + M + B)
work plus decompression/model-validation costs and O(B + P) retained storage. Existing
report grouping adds O(P log P) comparison work as an upper bound measured in metadata
bytes. No decompression-size quota is introduced. These bounds depend on A46.

Datetime adaptation scans L input bytes and retains O(L) temporary numeric/string
storage. Parsed date/time fields and formatted output have fixed bounds (years 1–9999,
microsecond precision, timezone offsets below 86,400 s). Library decimal conversion
costs depend on numeric input length; no new whole-document size bound is imposed.
These limits and the JSON lexical exclusions depend on A47.

Lazy bundle inspection loads central-directory metadata in O(N) entry operations and
retains O(P) member-name bytes. It reads/decompresses selected archive bytes only;
inner descriptor membership uses expected constant-time hash lookups plus path-byte
costs. Captured selected payloads require O(B) memory for B decompressed bytes. No
new decompression quota or whole-operation deadline is introduced. A48 bounds the
tested failure paths; these costs do not certify arbitrary ZIP resource behavior.

Local bundle resolution reads and hashes B aggregate archive bytes across P target
platforms. It retains U distinct archives with C total bytes and O(P) platform
assignments. Expected hash-map work is O(B + P), and retained storage is O(C + P)
plus the current read and descriptor-parsing costs. Literal existing paths take
one read. No read deadline, byte quota or snapshot guarantee is added under A49.

Repository bundle fetching reads and verifies B bytes across P platform selections,
then groups them using a second SHA-256 pass. Work is O(B + P), excluding repository
selection and decompression/model-validation costs. Retained archive storage is
O(C + P) for C distinct content bytes and platform assignments. Version naming scans
until the first matching descriptor. Native transport retains its existing 30 s
request deadline; this is not a whole-bundle deadline. These bounds depend on A50.

Reference parsing, fixed-pattern host matching, normalization and error rendering
take O(L) time and memory for L input bytes. Bundle preprocessing retains O(L) clean
spec/host text and adds linear scans before downstream version/repository work.
These bounds exclude account lookup, repository I/O and version matching under A51.

Snapshot deserialization and serialization traverse O(B) document/model bytes,
excluding metadata-validation costs, and retain O(B) model/output storage. Ordered
version maps use expected constant-time key operations plus key-byte hashing costs.
Selection sorts V version keys with O(V log V) comparisons and preserves the initial
order of equal-precedence keys. Hash verification processes O(A) archive bytes and
retains the existing O(A) download buffer. No new document/archive quota or overall
deadline is introduced under A52.

Snapshot formatting traverses B model bytes and emits R output bytes, including
indentation and ASCII escapes. Object sorting adds Σ Kᵢ log Kᵢ key comparisons for
objects with Kᵢ keys, plus string-comparison costs. Retained storage is O(B + R),
with a recursion stack proportional to nesting depth and temporary key lists.
The final integer-limit scan is O(R). No new output-size or nesting quota is added
under A53.

- QG1: No normative judgment is needed for this technical task.
- QG2: Assumptions and falsification probes are recorded above.
- QG3: Full requirement coverage remains open where the matrix identifies gaps.
- QG4: Timeout values are seconds; IPC bounds are bytes (1 MiB = 1,048,576 bytes).
- QG5: Unresolved edge cases are explicitly recorded; this gate remains open.
- QG6: Source provenance is pinned to local Git revisions.
- QG7: Additional findings are bounded to implementation and data preservation.

Full feature parity must not be claimed while QG3 or QG5 remains open.

# Hyper 1.10.1 local patch

Source: the published `hyper` 1.10.1 crate, upstream Git revision
`e3bcd379118e707b3e49185b047d49ebbaa943fc`.

The downloaded crate's SHA-256 is
`55281c53a1894c864990125767da440a4e630446785086f52523b20033b74498`,
matching the original Cargo.lock registry checksum.

The source tree, package manifests, README and MIT license are retained. The
crate-local rustfmt configuration preserves its upstream formatting separately
from Hy's configuration. The only implementation change is in
`src/proto/h1/role.rs`.

The exact delta is retained in [reason-phrase.patch](reason-phrase.patch), relative
to the unmodified crate. It uses zero-context hunks (`git apply --unidiff-zero`).

## Preserve non-ASCII HTTP/1 reason phrases

httparse 1.10.1 validates reason-phrase bytes but returns an empty string if any
byte is in the HTTP `obs-text` range (0x80–0xFF). Hyper previously copied that
empty string into its `ReasonPhrase` extension, irreversibly losing the wire
reason. CPython's http.client instead preserves it with Latin-1 decoding.

After httparse has validated a complete response, the patch recovers the raw
reason from the validated status line only when httparse returned an empty
string and the reason contains obs-text. Existing ASCII and empty
reasons use the original path. Status/header validation, framing and body
handling are unchanged. No new unsafe code is introduced.

The regression is exercised through real owned sockets in Hy's
`plugin::index::github::http::tests::wire_reasons_and_incomplete_error_bodies_keep_consumer_boundaries`.
It covers canonical, Latin-1, empty and Unicode-whitespace reason phrases with
truncated bodies and both GraphQL and search error policies. The full Hy suite
also exercises the shared HTTP dependency's other consumers.

When upgrading Hyper, compare its treatment of httparse's reason field and keep
the regression. Remove this patch when upstream preserves raw reason bytes.

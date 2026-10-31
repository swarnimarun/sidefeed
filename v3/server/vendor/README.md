# Vendored sax dependency

Bundled dependency: `sax` **1.6.1**, upstream https://github.com/isaacs/sax-js.
Pinned release commit: `961ceeefcc5186005466f2085aeb36f31b161e37` (commit message `1.6.1`, 2026-07-24).
Verified against upstream package.json, lib/sax.js and LICENSE.md at that exact commit on 2026-10-31. No npm install or lifecycle scripts executed.

`sax.cjs` is an **unmodified** copy of upstream `lib/sax.js`, renamed for Node ESM interoperability. Its local Git blob hash was verified equal to the upstream blob hash. `sax.package.json` records upstream package metadata; it is not used as this application's package manifest. Full BlueOak-1.0.0 license is in `sax.LICENSE.md` and must accompany distribution.

| Local file | Upstream Git blob SHA | SHA-256 |
| --- | --- | --- |
| sax.cjs | 3ae6728570523de9a800e0ec5cf20aac5a65cc42 | 1cdc1270d987cb0f6ee66dd8efb01c50511aa14d17b6163bbbffb21df8cf1c06 |
| sax.LICENSE.md | c5402b9577a8cdd966c0974e5b74cc7b85f536c5 | 8a1af140fdfbf5afd3df27f7e662f989c5b963a300020dfafce42033cae9e004 |
| sax.package.json | 40a81d08328594a5ac458456a844a7d7ab119183 | d7e5ff764b48b9532d1b9a8b481b8d54336e6309e362571e993645b790bea657 |

The application uses strict namespace parsing, strict predefined XML entities, no unparsed entity expansion, DTD rejection, byte/depth/node/attribute/text caps and chunked writes. It is a feed extraction adapter, not a complete RSS/Atom schema validator or full XML standard conformance claim. `test/feed-xml.test.mjs` covers supported extraction and rejection cases. Review upstream releases/security reports before changing the pin; preserve provenance and rerun tests. No claim of a complete third-party security audit is made.

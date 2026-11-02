# Effective configuration fixtures

Run `cargo test --locked -p ripple-resolve --test config`.

`root.jsonc` is captured as `apps/tsconfig.json`, extending `configs/base.json` supplied by `base.jsonc`. `lookup.json` independently specifies exact/wildcard precedence, target fallback, directory-relative requests and original source probe counts. Missing exact config probes and source candidates are retained. These are inventory-policy fixtures, not compiler-oracle results.

`fingerprint.json` supplies known raw/effective SHA-256 values. Reproduce independently with Python: hash the UTF-8 raw text, then hash `json.dumps(["ripple-config-v1-legacy-baseurl", "tsconfig.json", None, {}, [{"path":"tsconfig.json", "content_hash":{"version":1,"digest":raw_digest}, "byte_len":len(raw_bytes)}], []], separators=(",", ":"), ensure_ascii=False).encode()`.

The 21 integration tests cover declaration origins, child overrides/paths replacement, exact misses, ambiguous wildcard ties, JSONC strings/BOM/CRLF/trailing commas, duplicate decoded keys, missing-config repair, cycles/depth, unsupported policy, local containment, input/JSON bounds, source substitution, directory aliases, macros and invalid UTF-8. No external programs/project scripts run in these fixtures. Unsupported modes/configuration never produce a successful configured match.

# ADR 0002: Tree-sitter for syntax, separate supported resolution

Status: accepted (M0-T02).

## Context

Ripple needs TS/TSX syntax structure, source spans and recovery diagnostics. A syntax tree does not establish what an imported name or receiver denotes.

## Options

Text/regex extraction; invoking the full TypeScript compiler as the production parser/resolver; or Tree-sitter syntax extraction with a separately measured supported resolver.

## Decision

Use the pinned Tree-sitter runtime and TypeScript/TSX grammars in ripple-parse. Convert syntax into backend-independent FileFacts including lexical binding/scope evidence and explicit diagnostics. Keep name/module/call resolution in ripple-resolve. Planned declarations and unsupported cases are enumerated in architecture.md; the bounded parser wrapper is implemented in M1-T02. It selects grammar by case-sensitive extension, rejects invalid UTF-8 and inputs above 16 MiB, and stops through the runtime progress callback after a five-second budget. A cancelled parse has Failed status and no tree; syntax recovery has Recovered status with error/missing-node spans. Diagnostics are capped with an explicit truncation record.

## Reasoning

Structured parsing supports ownership/span mapping and error recovery while keeping the native CLI independent of a Node runtime. A separate resolver makes confidence rules measurable against the later TypeScript oracle rather than treating syntactic names as semantic proof.

## Consequences

Each supported grammar rule needs fixtures; grammar loads in bootstrap tests do not prove extraction quality. Failed syntax yields diagnostics without facts; partial recovery retains validated facts plus incomplete-analysis evidence. Hashes must preserve syntax structure and meaningful content, including automatic semicolon insertion.

## Revisit condition

Revisit grammar/runtime versions on a compatibility issue or measured unsupported syntax gap. Do not silently widen resolution confidence when syntax coverage grows.

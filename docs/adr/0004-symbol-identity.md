# ADR 0004: Structured repository-local semantic identities

Status: accepted (M0-T02).

## Context

Line numbers, DB row IDs, checkout locations and revisions change independently of a logical declaration. Names can collide between classes, overloads and quoted property keys.

## Options

Numeric storage IDs; line-based identities; concatenated display names; or structured semantic identity with unambiguous versioned encoding.

## Decision

Use validated project key, normalized relative path, structured qualified-name components, FUNCTION/CLASS/METHOD kind and optional stable disambiguator. Initial single-project key is persisted `default`. Encode canonical identity using versioned UTF-8 byte-length framing; dotted names are display only. Keep spans/revision/machine roots out of identity. Normalize portable slash separators; reject absolute/drive/UNC paths, traversal and control characters. Preserve case/Unicode spelling and diagnose scanner-level platform collisions.

Group supported overload signatures with their implementation; use stable declaration-role or signature-based disambiguation if necessary. Anonymous default declarations use an explicit default-export role. Never use line offsets as disambiguators. Git path renames may produce delete+add until reliable correspondence is implemented.

## Reasoning

Structural equality survives source-line shifts and separates methods with identical local names. Framing avoids separator/quoted-name collisions. Opaque DB row IDs remain free to change without affecting comparisons.

## Consequences

Changing identity encoding requires an analyzer/schema compatibility decision and rebuild. Paths remain part of identity, so moves do not automatically preserve it. Extractor tests must implement the documented grouping/disambiguation rules; contract tests alone do not prove their eventual correctness.

## Revisit condition

Revisit for a demonstrated identity collision, new supported declaration kind or proven move/rename matching—not for display formatting convenience.

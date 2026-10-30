# Current limitations

Ripple currently has core contracts and repository discovery; analysis commands are still being implemented. No resolver accuracy, complete graph, incremental correctness or test sufficiency is claimed.

Discovery uses one explicit project root, `.ts`/`.tsx`, repository `.gitignore`/`.ignore`, mandatory excludes and fixed test basename suffixes. It does not inherit parent/global Git ignores or `.git/info/exclude`. It skips all symlinks with diagnostics, rejects nonportable/case-colliding paths, and requires UTF-8 source and ignore text. Selected inputs are limited to 16 MiB, nesting to 128 and visited entries to 100000. Root tsconfig/package bytes are retained but their semantics are not yet interpreted. See [ADR 0010](adr/0010-source-discovery.md).

Worktree capture is not atomic under concurrent filesystem changes. Source/config/ignore manifest verification and bounded retry before index publication remain M2-T05 responsibilities. The immutable Git tree provider remains M4 work. No performance benchmark has been run.

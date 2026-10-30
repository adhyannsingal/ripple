# ADR 0010: Shared captured-input discovery

Status: accepted (M1-T01).

## Decision

`ripple-parse::scanner` discovers through a directory/byte `SourceTree` interface. The worktree adapter normalizes the explicit project root; a future Git adapter can supply immutable directory/blob data using the same policy. Discovery never invokes Git, project scripts or a resolver. The core SourceProvider implementation exposes captured UTF-8 sources; Discovery also retains source, root tsconfig/package and active ignore bytes for later manifest hashing.

Use pinned `ignore` 0.4.33 [GitignoreBuilder](https://docs.rs/ignore/0.4.33/ignore/gitignore/struct.GitignoreBuilder.html) to compile captured repository `.gitignore` and `.ignore` text. Process rules by directory, with child rules overriding parents and `.ignore` overriding same-directory `.gitignore`. Prune excluded parents before visiting descendants; negation cannot rescue a child of a pruned directory. Disable host-global, parent-project and `.git/info/exclude` inputs so revision/worktree policy is portable. Root package.json/tsconfig.json remain captured even if ignored. Capture ignores in every visited directory, including hidden directories; mandatory excludes cannot be negated.

Mandatory directory names: node_modules, .git, .ripple, dist, build, coverage, generated. Source extensions and test suffixes are case-sensitive. Only basename suffixes `.test.ts`, `.test.tsx`, `.spec.ts`, `.spec.tsx` classify tests. Hidden/untracked supported sources participate. Paths preserve spelling, but Unicode-lowercase path collisions fail explicitly for portability.

Skip all encountered symlinks, including internal links and directory cycles, with diagnostics. Reject nonportable filenames, special filesystem entries, malformed ignore rules, invalid UTF-8 source/ignore bytes and oversized selected inputs. Limit selected input reads to 16 MiB, directory nesting to 128 and visited entries to 100000. Root configuration bytes are captured without interpreting their syntax; config evaluation is a later task.

## Consequences

A project root is explicit rather than guessed by climbing parent directories. Unsupported encodings and skipped links cannot silently produce a complete graph claim. These checks prevent static symlink traversal but are not an OS-atomic snapshot or a sandbox against adversarial concurrent path replacement. Before publication the index layer must recapture membership/content and verify the manifest with bounded retries. No production Git adapter, input hashing, config interpretation or persistence is implemented here.

# ADR 0011: Shared captured-input discovery

Status: accepted (M1-T01), hardened during M1-T06. Renumbered to avoid the existing bootstrap ADR 0010.

## Decision

`ripple-parse::scanner` discovers through a directory/byte `SourceTree` interface. The worktree adapter normalizes the explicit project root; a future Git adapter can supply immutable directory/blob data using the same policy. Discovery never invokes Git, project scripts or a resolver. The core SourceProvider implementation exposes captured UTF-8 sources; Discovery also retains source, root tsconfig/package and active ignore bytes for later manifest hashing.

Use pinned `ignore` 0.4.33 [GitignoreBuilder](https://docs.rs/ignore/0.4.33/ignore/gitignore/struct.GitignoreBuilder.html) to compile captured repository `.gitignore` and `.ignore` text. Process rules by directory, with child rules overriding parents and `.ignore` overriding same-directory `.gitignore`. Prune excluded parents before visiting descendants; negation cannot rescue a child of a pruned directory. Disable host-global, parent-project and `.git/info/exclude` inputs so revision/worktree policy is portable. Regular root package.json/tsconfig.json files remain captured even if ignored; directories with those names receive ordinary ignore handling. Capture ignores in every visited directory, including hidden directories; mandatory excludes cannot be negated.

Mandatory directory names: node_modules, .git, .ripple, dist, build, coverage, generated. Source extensions and test suffixes are case-sensitive. Only basename suffixes `.test.ts`, `.test.tsx`, `.spec.ts`, `.spec.tsx` classify tests. Hidden/untracked supported sources participate. Paths preserve spelling, but Unicode-lowercase path collisions fail explicitly for portability.

Skip all encountered symlinks, including internal links and directory cycles, with diagnostics. Reject nonportable filenames, special filesystem entries, malformed ignore rules, invalid UTF-8 source/ignore bytes and oversized selected inputs. Limit selected input reads to 16 MiB, directory nesting to 128 and visited entries to 100000. SourceTree::entries receives the remaining budget; providers abort during iteration rather than returning truncated or unbounded listings. Root configuration bytes are captured without interpreting their syntax; config evaluation is a later task.

## Consequences

A project root is explicit rather than guessed by climbing parent directories. Unsupported encodings and skipped links cannot silently produce a complete graph claim. The worktree adapter opens the explicitly selected root once with pinned `cap-std` 4.0.3 and uses [directory-relative IO](https://docs.rs/cap-std/4.0.3/cap_std/fs/struct.Dir.html) thereafter. A renamed/replaced root pathname cannot redirect reads to a new tree. Check each encountered directory/file for symlinks; capability-relative opens constrain path replacement races, and the opened file handle must also be regular. Unix opens use rustix 1.1.6 constants for O_NONBLOCK/O_NOFOLLOW so FIFO substitution cannot wait for a writer. Core remains independent of these backend dependencies.

Initial root selection uses explicit ambient authority; later reads use the captured directory handle. This is not an OS-atomic content snapshot, a defense against hard links/mounts, or a guarantee against every concurrent filesystem mutation. The canonical root string is descriptive and can become stale after a rename. Before publication the index layer must recapture membership/content and verify the manifest with bounded retries. No production Git adapter, input hashing, config interpretation or persistence is implemented here.

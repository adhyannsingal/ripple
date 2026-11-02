# ADR 0020: Lexical runtime graph evidence

Status: implemented in M2-T02.

`symbols::resolve_graph` consumes captured validated facts, evaluated config and supplied scanner test classification. It emits typed file/symbol/opaque nodes, structural DEFINES, conditional configured IMPORTS/REEXPORTS and supported lexical CALLS/REFERENCES. No parser/IO/backend enters this resolver.

Walk actual lexical scopes to the first matching binding. Parameters and unsupported locals stop lookup, including destructuring/block shadowing. A unique represented local function/const-function declaration must enclose its binding span. Imports must have one configured binding, an original supported symbol and no type-only runtime restriction. Class reads may be references, but calling classes as functions, constructors, receivers, optional/computed calls and dynamic imports remain unresolved. No global method-name matching occurs. Flow-sensitive rebinding, complete JavaScript runtime behavior and Unicode identifier normalization are not modeled.

All targeted edges retain source ownership, original spans and a named rule. File-owned calls in test callbacks remain exact semantic evidence. File imports remain a separate conservative track. Unsupported references and failed module lookups create deterministic opaque occurrences with reasons and spans, never guessed targets. Static missing-module boundaries use the Read reference category with an explicit module-lookup reason. No candidate-producing rule is yet justified; ambiguous cases are unresolved. Existing graph contracts retain candidate confidence through serialization.

The graph token is ripple-graph-v1, composed with syntax/module/config versions by the index builder. Indexed scope/binding/import/declaration lookup avoids repeated full inventory scans. One million graph records is an explicit ceiling; exhaustion returns no graph. Graph construction validates typed endpoints/provenance. Performance and compiler-oracle accuracy are not measured.

Eight independent programs cover imports/aliases/barrels/defaults, local/const-arrow recursion, exact module-level test calls, reads, scope/destructuring shadowing, type-only imports, unsupported receiver/dynamic/constructor/optional forms, missing module boundaries, ordering and serialization. Run cargo test --locked -p ripple-resolve --test calls. No call target implies a sufficient test set or full runtime semantic proof.

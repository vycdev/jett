# Frontend Syntax Tree Staging

Status: decided direction; CST implementation deferred.

## Decision

Initial Jett versions use a direct, source-spanned syntax AST. The parser owns
this AST and the resolver, typechecker, interpreter, and future HIR consume it.
The current compiler does not have a lossless CST-to-AST lowering phase.

A later frontend version will introduce a lossless, error-tolerant CST and
lower it into the semantic AST without exposing CST details to later compiler
phases:

```text
initial: source -> tokens and comment trivia -> AST -> semantic phases
later:   source -> tokens -> CST -> AST -> semantic phases
```

The direct AST is the supported initial architecture, not a temporary parser
accident. The CST is a planned source-tooling layer, not a prerequisite for
settling initial language semantics.

## Future CST Requirements

- preserve every source byte, including comments and whitespace;
- produce a useful tree for malformed and incomplete files;
- give syntax nodes stable identities suitable for incremental parsing and
  structural agent edits;
- attach comments and documentation deterministically;
- remain syntax-only, with types and runtime policy kept in semantic phases;
- lower one-way into the AST so the resolver and later phases never depend on
  CST implementation details;
- preserve provenance from CST nodes through AST, HIR, MIR, diagnostics, and
  runtime debug events.

The provenance chain is important for agent tooling. CST data explains exact
source structure; type and ownership facts come from semantic analysis; runtime
behavior comes from lowered representations and execution. Debugging tools
should connect those layers rather than treating the CST as runtime metadata.

## Staging

1. Keep the current direct AST and token/comment-based formatter while initial
   semantics stabilize.
2. Establish the AST as the stable boundary consumed by semantic phases.
3. Add a dedicated syntax/CST layer when concrete needs such as structural
   edits, comment-preserving transformations, or incremental parsing justify
   it.
4. Lower CST to the existing AST and migrate formatter/LSP features
   incrementally.

The initial Salsa query and invalidation boundary must work with the direct AST
without pulling the deferred CST forward; that work is tracked by
[#147](https://github.com/vycdev/jett/issues/147).

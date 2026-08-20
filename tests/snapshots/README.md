# Parser AST snapshots

These fixtures exercise representative complete modules through the public
`jett_parser::parse` entry point. The committed `.snap` files live beside the
integration test under `crates/jett_parser/tests/snapshots/`, following
`insta`'s conventional layout.

Run the snapshots without changing them:

```bash
cargo test -q -p jett_parser --test ast_snapshots
```

After an intentional parser-tree change, regenerate and review them explicitly:

```bash
INSTA_UPDATE=always cargo test -q -p jett_parser --test ast_snapshots
git diff -- crates/jett_parser/tests/snapshots/
```

Only accept updates that reflect the intended AST change. Spans are byte offsets
within these fixed source files and every case uses `FileId::new(0)`, so the
output contains no absolute paths or process-assigned source identities.

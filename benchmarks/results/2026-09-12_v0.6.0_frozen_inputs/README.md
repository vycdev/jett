# v0.6.0 frozen benchmark inputs

`frozen-inputs.zip` preserves all 1,470 exact files named by the baseline and
campaign input manifests under `inputs/`, including the Git-ignored
`Cargo.lock`. It contains the compiler, stdlib, tasks, references, language
skills, grading helpers, and sandbox definition independently of later edits
or checkout newline conversion. It is source evidence, not a completed model
score report.

Every archived file's SHA-256 was verified against the frozen manifest, and
the current input snapshot was checked before and after archiving. Use an
isolated reproduction checkout when materializing these files; do not overwrite
unrelated local changes. The archive supplements source revision `bc6f96a` and
the pinned grading image; it does not replace that image.

Archive SHA-256:
`c894472df5bf71090c8bc3f617689ccd8f4d2a1aa502479306e14b25188f2af8`.

The archive was initially added as a supplement to the reference-validation
directory in commit `9169114`, then moved here with its bytes unchanged so it
has its own immutable result directory. The earlier reference-validation
README was restored; its original validation archive was never modified.
The original input manifest is retained in the published reference-validation
and interrupted-campaign evidence bundles.

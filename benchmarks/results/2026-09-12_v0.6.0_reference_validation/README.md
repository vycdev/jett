# v0.6.0 reference validation

This is reference-solution validation, not an LLM score report. The Luna
four-cell campaign and subsequent Jett skill revision remain in progress.

| Check | Coverage | Outcome |
| --- | ---: | --- |
| Local compiler/static/runtime checks | 500 unique task/language cells | 500 passed |
| First container checks | 500 unique cells | 499 passed, one Go compile timeout |
| Isolated recheck of that unchanged Go reference | 1 cell | passed |
| Combined container coverage | 500 unique cells across 501 retained attempts | 500 passed |

The timeout was `bounded_weighted_sum` v1.0.1 / Go during its compile phase.
The reference checker initially ran two workers sharing a 2-CPU container;
the operator then raised that container to 4 CPUs. Its unchanged source was
rechecked in a fresh, no-network, dedicated 2-CPU container, matching generated
submission resources, and passed in 12.043 seconds. Historical CPU sharing is
an operator-recorded fact, not a field independently captured in the result
rows. The original failure has not been replaced or removed.

An independent read-only audit checked all 1,001 retained reference-attempt
source hashes, unique task/language coverage, result-file receipt hashes, and
all 1,470 frozen input hashes against the workspace and grading image. No
mismatch was found. The 100-problem catalog includes 1,621 distinct shared
fixtures in its 90 additions, plus the original ten problems.

Frozen source revision: `bc6f96a`.
Grading image: `sha256:5cd341ed3f4062c4c56ab6d267111041aa375c2e39ae20db1770d2b46ee02e80`.

`validation-evidence.zip` preserves the local gate and manifests, every
original container row, the isolated recheck, and the combined verification
receipt. The archive preserves byte-exact files independently of Git newline
conversion. It contains no model responses or account credentials.

`frozen-inputs.zip` contains all 1,470 exact files named by the baseline and
campaign input manifests under `inputs/`, including the Git-ignored
`Cargo.lock`. It preserves the compiler, stdlib, tasks, references, language
skills, grading helpers, and sandbox definition independently of later edits
or checkout newline conversion. Every archived file's SHA-256 was checked
against the frozen manifest, and the current input snapshot was rechecked
before and after archiving. This supplements the source revision; it does not
replace the pinned grading image or claim that the model campaign is complete.

Frozen-input archive SHA-256:
`c894472df5bf71090c8bc3f617689ccd8f4d2a1aa502479306e14b25188f2af8`.

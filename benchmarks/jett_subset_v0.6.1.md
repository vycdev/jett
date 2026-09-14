# Jett benchmark subset v0.6.1

This is the separately versioned full-100-task Jett skill follow-up to the
published v0.6.0 four-cell campaign. It uses the same 100 public contracts,
adapters, compiler, graders, toolchains, and other language skills. Only the
general Jett language reference changes; exact input hashes establish parity.

The revised reference clarifies borrowing iteration, string interpolation and
implemented string signatures, result construction, and local nonzero proofs.
These are documented language rules, not evaluation algorithms or hidden-case
advice. The revision is development-set-informed, not a held-out treatment.

Use `config/jett_skill_v0.6.1.json` with the campaign preparer's `--language jett
--track skill_assisted` selection. First validate all 500 reference programs
under the new frozen input manifest. Generate 100 initial responses and exactly
one repair per failed initial response through the same ChatGPT-authenticated
Luna medium backend. Do not change initially passing answers or reuse old draws.

Keep the original report and image intact. Report both skill revisions
separately, with paired task gains, regressions, usage, and generated code size.
Do not pool revisions using the generic aggregate command.

# v0.6.0 interrupted campaign checkpoint

This is an incomplete development run, not a final language comparison.
The 1,000-prompt initial campaign stopped with 365 saved responses and three
additional interrupted attempt receipts. All 365 saved responses were graded
with the original immutable image; the canonical journal's first six grades
agree exactly with the corresponding staging grades. No saved candidate was
resampled or regraded to select a better result.

At the 2026-09-12 06:26:41 UTC audit, the original generator process and its
scoped children were absent. Exactly three receipts had no saved answer or
event trace. Their temporary directories were empty. The termination cause
and server-side completion are unknown. The original runner buffers event
output until subprocess completion or handled error, so no missing response,
usage, tool-use result, or completion event can be reconstructed from these
artifacts. Temporary-directory associations are timestamp-based inferences.

| Interrupted task | Language | Context |
| --- | --- | --- |
| `score_lines` | Rust | skill-assisted |
| `score_lines` | Python | no skill |
| `text_ipv4_parse` | Python | skill-assisted |

These are infrastructure interruptions, not failed programs eligible for a
compiler-feedback repair. Their input, cached-input, output, reasoning-token,
and latency metrics are unavailable, not zero. Completed-response token totals
must not be described as complete campaign consumption.

No replacement calls had been made at this checkpoint. A user decision was
requested before repeating these exact frozen prompts. Any authorized repeats
must retain and explicitly link these original attempts; no recorded successful
or failed candidate may be replaced. The campaign's automatic resume guard is
unchanged and still rejects unfinished prior attempts.

`checkpoint-evidence.zip` contains the authoritative campaign and its separate
staging copy, preserving byte-exact frozen plans, configuration, input hashes,
raw responses, event traces, attempt receipts, and grades. The stale generator
lock was moved without alteration into the campaign's `recovery/` directory
after verifying that its PID was absent. The interruption receipt records its
hash and the three original attempt hashes. It contains no account credentials.

Archive SHA-256:
`d209bc7e2b88606f0798693aec3945bfb558169af26f540b1b7b8653b11755bd`.
All 747 archived files were compared byte-for-byte by hash with the source
checkpoint before publication.

Frozen source revision: `bc6f96a`.
Image: `sha256:5cd341ed3f4062c4c56ab6d267111041aa375c2e39ae20db1770d2b46ee02e80`.
All 1,470 frozen input hashes remain unchanged. All 365 retained event hashes,
prompt identities, grade-to-response hashes, and grading-image identities were
verified before saving this archive.

The remaining initial responses, paired repairs, complete four-cell reports,
and full 100-task Jett skill-revision follow-up remain unfinished.

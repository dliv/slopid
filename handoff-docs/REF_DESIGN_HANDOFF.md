# Ref design handoff - recommended 1134 default

Date: 2026-05-31

This document is meant to stand alone in a fresh project. It captures the ref
design that appears to best match the owner's preferences after several rounds
of review. Treat this as the recommended default unless the owner explicitly
reopens the ref design.

It is not sacred scripture. It is the map of the territory already explored, so
the next agent does not painfully rediscover the same failure modes.

## Scope

A ref is the short `sXXXX` token inside:

```text
{YYYYMM}_{ref}[_{slug}]
```

Example:

```text
202605_sa2a7_fix-auth-state
```

The ref has to carry several jobs at once:

- be short enough for agents and humans to cite;
- help same-month folders sort in creation order in file browsers;
- look like a local slop-id token, not a git SHA or UUID;
- give enough monthly capacity for typical 20-80 task months and rare roughly
  200-task spikes;
- remain easy enough to test and explain.

## Recommended design

Use the 1134 mixed-radix design:

```text
ref = "s" + seq_hi + seq_lo + tail_hi + tail_lo
```

Alphabets:

```text
ALPHA22 = "abcdefghjkmnpqrtuvwxyz"
SLOP30  = "23456789abcdefghjkmnpqrtuvwxyz"
DIGITS  = "23456789"
START6  = "abcdef"
```

Notes:

- `ALPHA22` is lowercase ASCII letters minus ambiguous `i`, `l`, `o`, and minus
  `s` because `s` is already the ref prefix.
- `SLOP30` is `DIGITS` followed by `ALPHA22`.
- All generated refs are lowercase canonical.

Sequence encoding:

```text
seq range = 0..=659
seq_hi = ALPHA22[seq / 30]
seq_lo = SLOP30[seq % 30]
```

Examples:

```text
seq=0    -> a2
seq=1    -> a3
seq=7    -> a9
seq=8    -> aa
seq=29   -> az
seq=30   -> b2
seq=659  -> zz
```

Tail characters:

```text
tail_hi in SLOP30
tail_lo in SLOP30
```

Generation rule:

```text
if seq_lo is a digit:
  tail_hi must be in ALPHA22
else:
  tail_hi/tail_lo must contain at least one DIGITS character
```

In words: the four-character body after `s` must contain at least one digit, and
no generated ref may have an unbroken digit run crossing from `seq_lo` into
`tail_hi`.

## Generated and recognized refs

Generation should be strict. Recognition should be broader.

Generated refs are the union of these two cases:

```regex
# seq_lo digit: tail_hi breaks the digit run
^s[abcdefghjkmnpqrtuvwxyz][23456789][abcdefghjkmnpqrtuvwxyz][23456789abcdefghjkmnpqrtuvwxyz]$

# seq_lo letter: tail contains at least one digit
^s[abcdefghjkmnpqrtuvwxyz][abcdefghjkmnpqrtuvwxyz](?:[23456789][23456789abcdefghjkmnpqrtuvwxyz]|[23456789abcdefghjkmnpqrtuvwxyz][23456789])$
```

Allocator-recognized refs for scanning, occupancy, and sequence max:

```regex
^s[abcdefghjkmnpqrtuvwxyz][23456789abcdefghjkmnpqrtuvwxyz]{3}$
```

Examples:

```text
sa2a2  generated and recognized
sa222  recognized, not generated
saabc  recognized, not generated
s2a22  not recognized under 1134 because seq_hi is not ALPHA22
```

Why recognize more than is generated:

- hand-made or experiment-era folders should still reserve their refs;
- shape-compatible folders can still contribute to `max(seq)`;
- generation remains responsible for the chronological sort guarantee.

Document the limit clearly: generated refs are expected to sort correctly;
hand-made recognized-but-not-generated refs may not.

Loose agent/human recognition may be broader than allocator recognition. In
ordinary text, path, and chat contexts, a segment shaped like `_sXXXX_` should
look like a possible `sid` folder ref when the `X` characters are lower-case
alphanumerics. Allocator recognition is intentionally narrower because it needs
to decode the sequence and compute `max(seq)`.

## Deterministic month start

For the first allocation in an empty month, derive the starting sequence from
`YYYYMM` over the `START6 x START6` start space.

Recommended function:

```text
h = fnv1a32(ascii(YYYYMM))
slot = h % 36
seq_hi_start = START6[slot / 6]
seq_lo_start = START6[slot % 6]
seq_start = index(ALPHA22, seq_hi_start) * 30
          + index(SLOP30,  seq_lo_start)
```

Use standard 32-bit FNV-1a constants:

```text
offset = 2166136261
prime  = 16777619
```

Capacity with this start:

```text
min start = aa = 8
max start = ff = 163
usable/month = 497..652
```

Why deterministic start:

- keeps month-to-month visual variety;
- avoids hidden random state in the first folder of a month;
- avoids surprising re-rolls if all folders for a month are deleted;
- makes tests and examples reproducible.

If the owner decides this small visual signature is not worth the rule,
`seq_start = 0` is the clean fallback. Do not use a true random month start.

## Allocation algorithm

For `sid new`, use scan-then-create allocation:

```text
1. period = YYYYMM from local clock or an injected/test override.
2. Scan direct child directory names in task root plus configured scan dirs.
3. Keep current-period names with recognized 1134 ref shape.
4. Decode seq from seq_hi/seq_lo.
5. seq = if no siblings then deterministic seq_start(period)
         else max(decoded seq) + 1.
6. If seq > 659, fail "monthly sequence exhausted".
7. For attempts 1..=100:
     draw tail_hi/tail_lo uniformly from SLOP30 x SLOP30
     if seq_lo is digit and tail_hi is digit: retry
     if seq_lo is letter and tail has no digit: retry
     ref = "s" + encode_seq(seq) + tail
     if the same period/ref is occupied in any scan dir: retry
     mkdir {task-root}/{period}_{ref}[_slug]
       success: return
       AlreadyExists: retry
       other error: fail
8. Fail after the attempt budget.
```

Important rules:

- The sequence value is fixed for one invocation after scanning.
- There is no pin-at-max behavior.
- There is no digit-rule-off behavior.
- Slug never disambiguates an occupied ref.
- No lock file in v1; concurrency is best-effort through atomic `mkdir`.
- Generated refs are lowercase canonical.

## Occupancy

A ref is occupied for a period if any configured scan dir has a direct child
named either:

```text
{YYYYMM}_{ref}
{YYYYMM}_{ref}_{slug}
```

An occupied ref is not available even if the new slug would make the full folder
name distinct. This preserves the operational promise that the short ref usually
points to one task.

## Agent-facing model

Agents should have to think about the format minimally:

- Use `sid new "title"` to start a feature or task folder.
- Cite and search the full `sXXXX` ref, not bare fragments.
- Use `sid` lookup/listing when convenient; ordinary text search such as `rg
  sXXXX` is also fine.
- On rare `sXXXX` collisions, resolve by nearby context: newest `YYYYMM`, slug,
  surrounding task context, or ask the user if still ambiguous.
- Do not invent refs by hand for durable task folders. Let `sid` allocate them.

## Sorting rationale

The owner cares about the folder tree as a UI. Same-month folders should appear
in allocation order in Finder, VS Code, and simple listings as much as possible.

The 1134 design supports that by putting the monotonic sequence in the first two
characters after `s`:

- `seq_hi` is always a letter, so the first sort-bearing character is never a
  digit.
- `seq_lo` follows `SLOP30` order.
- if `seq_lo` is a digit, `tail_hi` is forced to a letter so natural sort cannot
  merge the sequence digit with a digit-leading tail.
- if `seq_lo` is a letter, any digit in the tail starts after a letter and cannot
  merge with the sequence.

Earlier review found no generated 1134 refs that sort out of allocation order
under bytewise sort or conservative Finder/VS Code style natural-sort models.
Some real-tool checks disagreed about whether the older raw slop30 failure
reproduces in every comparator. That disagreement is exactly why this design
avoids depending on comparator-specific digit-run behavior.

## Why digit-anywhere wins

Generated refs should contain at least one digit somewhere in the four-character
body after `s`.

Do not require a digit specifically in the random tail when `seq_lo` is already
a digit.

Rejected stricter rule:

```text
if seq_lo is digit:
  tail_hi must be ALPHA22
  tail_lo must be DIGITS
```

Why rejected:

- the full `sXXXX` ref is what agents cite, and a digit in `seq_lo` is visible;
- the stricter rule has no sorting benefit;
- accepted tails in digit-seq bands drop from `660 / 900` to `176 / 900`;
- expected draws for those bands rise from about `1.36` to about `5.11`;
- it reduces same-sequence collision headroom under best-effort concurrency.

## Concurrency

Do not add a lock file in v1 unless the owner explicitly changes the target.

Accepted limitation:

- two simultaneous `sid new` processes can scan the same state and choose the
  same sequence;
- if they draw the same full ref, one wins the exact `mkdir` race and the other
  retries;
- more often, they draw different tails, so same-sequence folders may both be
  created and then sort by tail inside that small concurrent group;
- strict cross-process chronological ordering is not guaranteed.

This is acceptable for a local, single-user tool. If real usage shows painful
parallel allocation, revisit a small fail-loud lock later.

## Candidate designs not chosen

### Raw slop30 sequence

Shape:

```text
ref = "s" + slop30_seq2 + slop30_tail2
```

Why it was attractive:

- very simple;
- high capacity;
- pure slop30 aesthetic.

Why it lost:

- mixed digit/letter sequence chars can be risky under natural sort;
- the canonical example is `s2922` vs `s2a22`, where a greedy natural-sort model
  can treat digit runs in a way that inverts allocation order;
- real tools were not perfectly consistent in review, which makes relying on raw
  slop30 too brittle for the folder-tree UI goal.

### SEQ16

Shape:

```text
seq2 = base-16 over "ghjkmnpqrtuvwxyz"
tail2 = SLOP30 x SLOP30, with a digit in the tail
```

Why it was attractive:

- very simple;
- natural-sort-safe by construction;
- sequence chars are structurally non-hex.

Why it lost:

- it over-optimizes the "not a git SHA" concern;
- the full ref starts with `s`, so agents should not confuse `sXXXX` with a git
  commit hash;
- 256/month is probably enough, but it is a tighter ceiling than necessary.

### SEQ22

Shape:

```text
seq2 = base-22 over ALPHA22
tail2 = SLOP30 x SLOP30, with a digit in the tail
capacity = 484/month before any start offset
```

Why it remains the best fallback:

- sort-safe by construction;
- much simpler than 1134;
- enough capacity for the stated workload.

Why 1134 is still the recommended default:

- it preserves slop30 digit texture in the sequence field;
- it gives roughly 497..652 usable refs/month with the deterministic start;
- the extra combo-breaker rule is small and testable;
- this is the trade-off the owner chose after review.

If simplicity wins later, choose SEQ22 rather than going back to SEQ16.

### Pin-at-max behavior

Old idea:

```text
seq = min(max(seq) + 1, max_seq)
at max_seq, keep reusing the max sequence and only change tail
```

Why it lost:

- it creates special cases around exhaustion;
- it invites digit-rule exceptions;
- it makes tests and docs harder;
- it solves a monthly volume problem outside the target use case.

Prefer a plain exhaustion error when `seq > 659`.

### Slug-last-resort

Old idea: allow duplicate refs if the slug makes the full folder name distinct.

Why it lost:

- it weakens the promise that a short ref usually identifies one task;
- it makes `sXXXX` search ambiguous exactly when the namespace is stressed;
- it adds retry-loop ambiguity;
- the slug is supposed to be context, not identity.

Slug must not disambiguate an occupied ref.

### Pure random refs

Why it lost:

- loses same-month chronological sorting in the folder tree;
- turns discovery into search rather than ordered browsing.

### Variable-length refs

Why it lost:

- hurts grep and agent citation;
- complicates exact-vs-prefix search;
- makes parsing and docs more fiddly than the problem deserves.

### Decimal sequence

Example:

```text
ref = "s" + three_digit_seq + one_tail_char
```

Why it was considered:

- fixed-width decimal sequence is sort-safe in many natural-sort tools;
- capacity can be high.

Why it lost:

- it looks like a serial ticket counter;
- it loses much of the intended slop-id texture;
- with only one tail char, random visual variety is weaker;
- if the tail can be a digit, it can create its own natural-sort boundary issues.

## Stateless `sid id`

If a public `sid id` command exists, document it carefully.

It does not scan. It does not reserve a ref. It does not promise chronological
folder ordering if users later materialize ids manually.

Two possible choices:

- defer `sid id` until a concrete non-folder workflow needs it;
- keep it as "generate a syntactically valid id string only" and make the docs
  explicit that durable task folders should use `sid new`.

Do not let `sid id` blur the semantics of allocated refs.

## Implementation testing notes

Tests should lock:

- alphabet membership and order;
- `encode_seq` / `decode_seq` examples;
- generated-vs-recognized ref distinction;
- deterministic `seq_start(period)` test vectors once implemented;
- allocation from empty month;
- allocation from existing max sequence;
- monthly exhaustion at `seq > 659`;
- occupied refs across active, pending, archive, and additional scan dirs;
- no slug fallback for occupied refs;
- `AlreadyExists` retry behavior;
- JSON output fields once CLI shape is chosen.

The next agent should start from this design, ask clarifying questions, and then
TDD the observable behavior.

<human>do not dive straight into detailed questions. we'll discuss high level first. likely use the `/grill-me` skill</human>

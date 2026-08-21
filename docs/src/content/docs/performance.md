---
title: Performance model
description: Why locguard stays fast by avoiding files, bytes, parsing, and unnecessary state.
order: 5
category: Reference
summary: The threshold-scanning design and the optimizations that matter most.
---

Locguard optimizes for one predicate:

> Does this source file contain more than its configured number of physical lines?

It is not trying to calculate semantic code metrics.

## Do less work

The implementation optimizes in this order:

1. Do not consider irrelevant files.
2. Do not consider unchanged files when changed mode is sufficient.
3. Do not read bytes after a violation has been proven.
4. Scan independent files with modest parallelism.
5. Reuse buffers and minimize allocation/syscall overhead.
6. Count newline bytes efficiently.

## Git-aware discovery

Inside Git, locguard lets Git own tracked/untracked and ignore semantics. Bare checks use read-only porcelain status; full scans enumerate tracked plus nonignored untracked paths.

This also means locguard automatically benefits when a user has Git's untracked cache or filesystem monitor enabled without changing Git configuration itself.

## Early termination

With a 1,000-line limit, a 50,000-line offender does not need to be read to EOF. Once physical line 1,001 is proven, default scanning stops and reports:

```text
Current LOC limit: 1000 lines, 1 file exceeded this limit.
FAIL src/monster.rs
```

Use `--exact` only when the exact offender count is actually useful.

## Free file-size proof

Locguard already reads file metadata so it can reject symlinks safely. That makes byte length free information: a physical line requires at least one byte, so a file whose byte length is below its warning threshold cannot possibly warn or fail.

Locguard still opens the file so unreadable source remains an operational error, but it can skip the content read entirely for these small files.

## Physical lines, directly from bytes

Comments and blank lines count. For ordinary source bytes, UTF-8 decoding and language parsers are unnecessary; line boundaries are determined by newline bytes, with a final unterminated line counted normally.

If the first 64 KiB of an otherwise eligible source path contains a NUL byte, locguard treats it as binary-like and skips it. That avoids inventing encoding semantics for UTF-16 or binary fixtures while keeping the scanner parser-free.

This keeps the hot path portable and language-independent.

By default locguard uses up to eight workers, capped by the machine's available
parallelism. Benchmarks on large source trees showed eight workers substantially
outperforming four on the 10,000-file stress case, while higher concurrency began
to regress. Use `-j/--threads` when a particular filesystem benefits from a
different level of I/O parallelism.

## No persistent cache in v1

The scanner intentionally has no mtime/path cache or daemon. Repeated checks stay trustworthy and stateless. A content-addressed Git-blob cache would only be worth adding if measurements on genuinely huge repositories showed the simpler scanner was insufficient.

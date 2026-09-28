# Project Issues & Code Review Tracker

This file tracks known bugs, correctness gaps, and code-quality improvements for this
Redis implementation (CodeCrafters "Build your own Redis" in Rust). It is the shared
channel between the project owner and any AI assistant reviewing the code.

**Last reviewed:** 2026-09-29 (commit `78b4ffa` + uncommitted parser rewrite; stages passed at last commit: up to *Expiry*)

---

## Instructions for AI assistants

If you are an AI analyzing or modifying this codebase, you **must** keep this file up to date:

1. **Read this file first** before reviewing or changing code, so you know what's already known.
2. **New issue found?** Add it to the correct section below using the next free ID
   (IDs are never reused). Follow the entry template at the bottom of this file.
   Don't add duplicates. If an existing entry covers it, update that entry instead.
3. **Issue fixed in the code?** Whether you fixed it or you notice it has been fixed:
   - Change its status to `Fixed` and add a one-line note of how it was fixed and the commit (if known).
   - Move it to the **Resolved** section at the bottom.
4. **Issue no longer applies** (code removed/rewritten, or it turned out to be wrong)? Mark it
   `Won't fix` or `Invalid` with a short reason and move it to **Resolved**.
5. **Locations drift.** Line numbers are hints only. Always verify against the current code, and
   refresh the location when you touch an entry.
6. Update the **Last reviewed** line at the top whenever you do a review pass.
7. **Do not change application code just because an issue is listed here.** Only fix issues when
   the project owner asks you to. This file is for tracking, not a to-do list for the AI.
8. Keep entries short and concrete: what's wrong, how to reproduce it, and a suggested fix.

**Status values:** `Open` · `In progress` · `Fixed` · `Won't fix` · `Invalid`
**Severity values:** `High` (crash / wrong protocol output) · `Medium` (wrong behavior vs Redis) · `Low` (quality / idiom)

---

## Summary

The parser has been rewritten (see **Resolved**): it returns `Result`, reports `Incomplete` for
partial input, and no longer panics on the tested edge cases. `main.rs` and `executor.rs` still
use the old API, so the project does not build yet.

Next: update `main.rs`/`executor.rs` → BUG-001 (persistent read buffer) → BUG-008 (bytes) → the rest.

---

## Bugs (confirmed or high-confidence)

### BUG-001: No buffering across socket reads
- **Status:** Open
- **Severity:** High
- **Location:** `src/main.rs` — `handle_connection` (fixed `[0; 1024]` buffer)
- **Problem:** TCP is a byte stream, so one `read` can end mid-command. Leftover partial bytes are
  thrown away, and any command larger than 1024 bytes can never be parsed.
- **Repro (verified):** Send `*1\r\n$4\r\nPI` and then `NG\r\n` as two writes. The server panics at
  `parser.rs` (slice out of bounds) and the connection drops. Same for `SET a <1100-byte value>`.
- **Suggested fix:** Keep a `BytesMut` per connection (the `bytes` crate is already a dependency).
  Append each read to it, parse as many complete frames as possible, and keep the remainder for
  the next read. The parser needs to be able to report "incomplete" (see BUG-004).

### BUG-002: `-ERR invalid request` branch is unreachable
- **Status:** Open
- **Severity:** Medium
- **Location:** `src/main.rs` — `else` branch after `parser::parse` in `handle_connection`
- **Problem:** `parse` only returns `None` when `start > end`, which the `while start < bytes_read`
  loop already prevents. Malformed input panics instead of reaching this branch.
- **Suggested fix:** Resolves naturally once the parser returns a proper `Result` (BUG-004).

### BUG-008: `from_utf8_lossy` corrupts binary data and breaks ECHO framing
- **Status:** Open
- **Severity:** High
- **Location:** `src/parser.rs` — `b'$'` branch; `src/executor.rs` — `ECHO`
- **Problem:** Redis strings can hold any bytes. Invalid UTF-8 bytes become U+FFFD (3 bytes each),
  but `ECHO` still writes the original length, so the reply's framing is broken.
- **Repro (verified):** ECHO `\xff\xfe` (2 bytes) replies `$2\r\n` followed by 6 bytes.
- **Suggested fix:** Store bulk strings and values as `Vec<u8>` / `Bytes`, not `String`.

### BUG-011: Invalid or unsupported `SET` options are silently ignored
- **Status:** Open
- **Severity:** Medium
- **Location:** `src/executor.rs` — `SET` option loop
- **Problem:** `SET k v EX abc` stores the key with no expiry and replies `+OK` (Redis returns an
  error). `EX 0` or negative values should be errors. Unknown options (`NX`, `XX`, `GET`,
  `KEEPTTL`) are skipped silently, and a missing value after `EX`/`PX` is ignored.
- **Suggested fix:** Return `-ERR syntax error` / `-ERR invalid expire time in 'set' command`
  as Redis does.

### BUG-012: `secs * 1000` can overflow
- **Status:** Open
- **Severity:** Low
- **Location:** `src/executor.rs` — `"EX"` arm
- **Problem:** Very large values panic in debug builds.
- **Suggested fix:** Use `checked_mul`, or build a `Duration::from_secs` directly.

### BUG-013: Missing arguments return the wrong error
- **Status:** Open
- **Severity:** Low
- **Location:** `src/executor.rs` — fall-through to `"-ERR invalid request"`
- **Problem:** `GET` with no key replies `-ERR invalid request`. Redis replies
  `-ERR wrong number of arguments for 'get' command`.
- **Suggested fix:** Check the argument count per command and return the Redis-style error.

### BUG-014: Expired keys are only removed when someone reads them
- **Status:** Open
- **Severity:** Low (fine for current stages)
- **Location:** `src/data_manager.rs` — `get`
- **Problem:** An expired key that is never read again stays in memory forever.
- **Suggested fix:** Add a periodic background task that samples keys and deletes expired ones,
  like Redis does.

### BUG-015: Replication RDB transfer has no trailing CRLF
- **Status:** Open
- **Severity:** Medium (only matters once the replication extension starts)
- **Location:** `src/parser.rs` — `b'$'` branch
- **Problem:** In the replication handshake the master sends the RDB file as
  `$<len>\r\n<binary contents>` with **no** trailing `\r\n`. The `$` branch requires the CRLF,
  so it will report this as `Invalid` or `Incomplete`.
- **Suggested fix:** Add a separate parse path for the RDB payload, used only at that point in the
  handshake.

### BUG-016: Simple errors (`-`) are not parsed
- **Status:** Open
- **Severity:** Low (clients never send errors; matters when acting as a replica)
- **Location:** `src/parser.rs` — `_` branch
- **Problem:** `RespValue::Error` exists but the parser never produces it. `-ERR ...` input
  is rejected as an unexpected type byte.
- **Suggested fix:** Add a `b'-'` branch mirroring the `b'+'` branch.

### BUG-017: Unlimited nesting depth in the parser
- **Status:** Open
- **Severity:** Low
- **Location:** `src/parser.rs` — recursive call in the `b'*'` branch
- **Problem:** `*1\r\n*1\r\n...` recurses once per level. Harmless with the fixed 1024-byte buffer,
  but with a growing buffer (BUG-001) a client could send enough nesting to overflow the stack.
- **Suggested fix:** Pass a depth counter and return `Invalid` past a limit (e.g. 128).

### BUG-018: Large commands are re-parsed from the start on every read
- **Status:** Open
- **Severity:** Low (performance only)
- **Location:** `src/parser.rs` / `src/main.rs` read loop
- **Problem:** On `Incomplete`, the next read parses the whole command again from the beginning.
  Correct, but quadratic for very large payloads arriving in many small pieces.
- **Suggested fix:** Only worth doing if it shows up in practice, e.g. check a bulk string's
  declared length against the buffered bytes before re-parsing.

---

## Code quality / idiomatic Rust

### QUAL-004: Encode responses in one place
- **Status:** Open
- **Location:** `src/executor.rs`
- **Note:** Each command builds its reply with `format!`. Return a `RespValue` and write one
  `encode()` / `serialize()` function. That puts the protocol format in one spot and prevents
  BUG-008-style length mismatches.

### QUAL-005: `is_some()` followed by `unwrap_or(0)`
- **Status:** Open
- **Location:** `src/executor.rs` — after the SET option loop
- **Note:** Use `if let Some(ms) = expiry_ms { ... }`.

### QUAL-006: `#[allow(clippy::collapsible_if)]` hides a lint that could be fixed
- **Status:** Open
- **Location:** `src/executor.rs` — above `execute`
- **Note:** Let-chains are already used (edition 2024), so collapse the nested `if let`s and
  remove the `allow`.

### QUAL-007: `to_uppercase()` allocates on every command
- **Status:** Open
- **Location:** `src/executor.rs`
- **Note:** Use `eq_ignore_ascii_case` or `to_ascii_uppercase()`. Minor.

### QUAL-008: `execute` will keep growing as commands are added
- **Status:** Open
- **Location:** `src/executor.rs`
- **Note:** Split it into one function per command, e.g. `fn set(args: &[RespValue], db: &Db) -> RespValue`.

### QUAL-009: Simplify the expiry check in `DataManager::get`
- **Status:** Open
- **Location:** `src/data_manager.rs` — `get`
- **Note:** `self.data.get(key).and_then(|d| d.expiration).is_some_and(|t| Instant::now() >= t)`.
  The look up → remove → look up again pattern is a valid way around a borrow-checker limitation.

### QUAL-010: Values are plain `String`
- **Status:** Open
- **Location:** `src/data_manager.rs` — `RedisData`
- **Note:** Later stages (lists, streams) will need an enum of value types. Also see BUG-008 (bytes).

### QUAL-011: Unused dependencies
- **Status:** Open
- **Location:** `Cargo.toml`
- **Note:** `anyhow`, `thiserror` and `bytes` aren't used yet. Use them (see BUG-001, BUG-004) or
  remove them.

### QUAL-012: No unit tests
- **Status:** Open
- **Location:** `src/parser.rs` (highest value)
- **Note:** The parser is easy to test on its own. Tests for split input, null bulk strings,
  binary data and malformed lengths would have caught most of the bugs above.

---

## What's already good (keep doing this)

- `std::sync::Mutex` in async code is correct here, because the lock is never held across an `.await`.
- `Arc<Mutex<…>>` plus one spawned task per connection is the standard setup.
- The builder style in `RedisData::new(...).with_expiration(...)` is idiomatic.
- `Instant` (monotonic time) for expiry rather than wall-clock time.
- Multiple complete commands in one read are handled (basic pipelining).

---

## Resolved

### BUG-003: Redundant `end` parameter on `parse`
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** `parse(input: &[u8])` takes only a slice; positions are relative to it. (`main.rs` call site still to be updated.)

### BUG-004: Parser has no bounds checks and panics on short input
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** `parse` returns `Result<(RespValue, usize), ParserError>`; missing CRLF or a short bulk body returns `ParserError::Incomplete` instead of panicking.

### BUG-005: `convert_byte_to_int` doesn't validate digits and overflows
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** `convert_byte_to_int` removed; `parse_int` uses `str::parse::<i64>()`. `$-1` → `Null`, `*-1` → `NullArray`, other negatives → `Invalid`, bulk length capped at 512 MB.

### BUG-006: Array parsing silently drops elements that fail to parse
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** Array elements are parsed with `?`, so any element error propagates.

### BUG-007: Trailing `\r\n` is never verified
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** Header lines end at the found CRLF; the bulk string branch verifies the trailing `\r\n` and returns `Invalid` otherwise.

### BUG-009: Fallback (inline) branch decodes bytes as Latin-1
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** Fallback branch removed; an unknown type byte now returns `Invalid("unexpected type byte: ..")`. Inline commands are not supported (the tester doesn't send them).

### BUG-010: `todo!()` for `:` and `+` lets clients crash the connection
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** `:` parses to `RespValue::Integer` and `+` to `RespValue::SimpleString`; no `todo!()` left.

### QUAL-001: Use `Result` instead of `Option` plus panics in the parser
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** Done together with BUG-004 (`ParserError { Incomplete, Invalid(String) }`).

### QUAL-002: Use slices and std parsing instead of index arithmetic
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** Uses `windows(2).position(..)` to find the line end, a `parse_int` helper and `let ... else`.

### QUAL-003: Rename `RedisRes` and remove the redundant length field
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. Project does not build until `main.rs`/`executor.rs` are updated, so stages not re-run yet)
- **Fix:** Renamed to `RespValue`; length fields removed; `Null`/`NullArray`/`Integer`/`Error` variants added. (`executor.rs` still to be updated.)

<!--
Example of a resolved entry:

### BUG-0XX: Short title
- **Status:** Fixed (2026-10-01, commit abc1234)
- **Fix:** One line on what changed.
-->

---

## Entry template

```markdown
### BUG-0XX / QUAL-0XX: Short title
- **Status:** Open
- **Severity:** High | Medium | Low   (bugs only)
- **Location:** `src/file.rs` — function or code landmark
- **Problem:** What is wrong and why it matters.
- **Repro:** Exact input that triggers it (if applicable). Say whether it was verified.
- **Suggested fix:** Short, concrete direction.
```

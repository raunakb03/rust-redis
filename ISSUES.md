# Project Issues & Code Review Tracker

This file tracks known bugs, correctness gaps, and code-quality improvements for this
Redis implementation (CodeCrafters "Build your own Redis" in Rust). It is the shared
channel between the project owner and any AI assistant reviewing the code.

**Last reviewed:** 2026-09-28 (at commit `78b4ffa`, stages passed: up to *Expiry*)

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

The core problem is the **parser**: it assumes every socket read contains complete, well-formed
commands. When that isn't true it panics instead of returning an error. Fixing BUG-001 and
BUG-004 through BUG-008 together (a `Result`-returning parser, a persistent read buffer, and
byte-based values) resolves most of the High-severity items.

Suggested order: BUG-001 → BUG-004 → BUG-008 → the rest.

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

### BUG-003: Redundant `end` parameter on `parse`
- **Status:** Open
- **Severity:** Low
- **Location:** `src/main.rs` call site, `src/parser.rs` `parse` signature
- **Problem:** `bytes_read - 1` is passed alongside `&buf[..bytes_read]`. It duplicates
  `input.len()` and is an off-by-one hazard.
- **Suggested fix:** Drop `end` and use `input.len()` (or parse from a slice/cursor).

### BUG-004: Parser has no bounds checks and panics on short input
- **Status:** Open
- **Severity:** High
- **Location:** `src/parser.rs` — `parse` (`while input[start] != b'\r'`, `&input[start..start + len]`)
- **Problem:** Indexing assumes data is complete, so truncated input panics.
- **Suggested fix:** Return `Result<(Frame, usize), ParseError>` with
  `enum ParseError { Incomplete, Invalid(String) }` (`thiserror` is already a dependency).
  Check bounds before every access.

### BUG-005: `convert_byte_to_int` doesn't validate digits and overflows
- **Status:** Open
- **Severity:** High
- **Location:** `src/parser.rs` — `convert_byte_to_int`
- **Problem:** A `-` or other non-digit underflows (panics in debug, silently wrong in release).
  Large lengths overflow `u32`. Negative lengths (`$-1` null bulk string, `*-1` null array)
  aren't supported.
- **Repro (verified):** `*2\r\n$4\r\nECHO\r\n$-1\r\n` panics with subtraction overflow.
- **Suggested fix:** Find the line end, then `std::str::from_utf8(line)?.parse::<i64>()`.
  Handle `-1` explicitly.

### BUG-006: Array parsing silently drops elements that fail to parse
- **Status:** Open
- **Severity:** Medium
- **Location:** `src/parser.rs` — `b'*'` branch, `for _ in 0..num_eles` loop
- **Problem:** If an element returns `None`, the loop continues and returns a shorter array as if
  nothing went wrong.
- **Suggested fix:** Propagate the error (`?`) instead of skipping.

### BUG-007: Trailing `\r\n` is never verified
- **Status:** Open
- **Severity:** Low
- **Location:** `src/parser.rs` — every `start += 2`
- **Problem:** The code skips two bytes without checking that they are `\r\n`.
- **Suggested fix:** Verify `&input[i..i + 2] == b"\r\n"` or return `ParseError::Invalid`.

### BUG-008: `from_utf8_lossy` corrupts binary data and breaks ECHO framing
- **Status:** Open
- **Severity:** High
- **Location:** `src/parser.rs` — `b'$'` branch; `src/executor.rs` — `ECHO`
- **Problem:** Redis strings can hold any bytes. Invalid UTF-8 bytes become U+FFFD (3 bytes each),
  but `ECHO` still writes the original length, so the reply's framing is broken.
- **Repro (verified):** ECHO `\xff\xfe` (2 bytes) replies `$2\r\n` followed by 6 bytes.
- **Suggested fix:** Store bulk strings and values as `Vec<u8>` / `Bytes`, not `String`.

### BUG-009: Fallback (inline) branch decodes bytes as Latin-1
- **Status:** Open
- **Severity:** Low
- **Location:** `src/parser.rs` — `_` branch (`input[start] as char`)
- **Problem:** Non-ASCII text comes out garbled.
- **Suggested fix:** Work with bytes, or decode the whole line with `from_utf8`.

### BUG-010: `todo!()` for `:` and `+` lets clients crash the connection
- **Status:** Open
- **Severity:** High
- **Location:** `src/parser.rs` — `b':'` and `b'+'` branches
- **Repro (verified):** Sending `+PING\r\n` panics.
- **Suggested fix:** Implement them, or return `ParseError::Invalid`. Client input must never panic.

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

---

## Code quality / idiomatic Rust

### QUAL-001: Use `Result` instead of `Option` plus panics in the parser
- **Status:** Open
- **Location:** `src/parser.rs`
- **Note:** See BUG-004. This lets the connection loop tell "wait for more bytes" apart from
  "send an error".

### QUAL-002: Use slices and std parsing instead of index arithmetic
- **Status:** Open
- **Location:** `src/parser.rs`
- **Note:** `input.windows(2).position(|w| w == b"\r\n")` to find line ends, then
  `str::parse::<i64>()`. This removes `convert_byte_to_int` entirely.

### QUAL-003: Rename `RedisRes` and remove the redundant length field
- **Status:** Open
- **Location:** `src/parser.rs`
- **Note:** `RedisRes` is used for parsed *requests*. `RespValue` or `Frame` is clearer. The
  `usize` in `String(usize, String)` / `BulkString(usize, String)` duplicates the string's own
  length. Clippy reports the `String` variant's fields and the `None` variant as never used.

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

_Nothing yet. Move entries here when they're fixed, marked `Won't fix`, or found `Invalid`._

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

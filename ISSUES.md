# Project Issues & Code Review Tracker

This file tracks known bugs, correctness gaps, and code-quality improvements for this
Redis implementation (CodeCrafters "Build your own Redis" in Rust). It is the shared
channel between the project owner and any AI assistant reviewing the code.

**Last reviewed:** 2026-10-14 (uncommitted `RPUSH` work on top of `08d7519`; clippy clean, 50 unit tests and all 8 stages pass; `cargo fmt --check` still fails)

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

The parser and the connection loop have been rewritten, and `executor.rs` restructured into one
function per command that returns a `RespValue` (see **Resolved**). The project builds and all stages up to *Expiry* pass.
The remaining items are mostly for later extensions or code quality; none are tested by the
current stages.

Next: commit → Lists extension (starting with QUAL-010). Run `cargo test` and `./test` after every change.

---

## Bugs (confirmed or high-confidence)

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

### BUG-020: `SET` accepts conflicting or repeated expiry options
- **Status:** Open
- **Severity:** Low
- **Location:** `src/executor.rs` — `set`, options loop
- **Problem:** `SET k v EX 10 PX 100` (or `EX 1 EX 2`) silently uses the last option. Redis replies
  `ERR syntax error` when more than one expiry option is given.
- **Repro:** Read from the code, not run against the server.
- **Suggested fix:** If `ttl` is already `Some` when an `EX`/`PX` is seen, return `ERR syntax error`.

### BUG-021: Connection buffer limit is both too strict and too loose
- **Status:** Open
- **Severity:** Low
- **Location:** `src/main.rs` — `handle_connection`, `buffer.len() > parser::MAX_BULK_LEN`
- **Problem:** The limit applies to the whole buffer, so a legal 512 MB value plus its RESP header
  and the rest of the command is rejected. At the same time, a client that sends bytes with no
  `\r\n` makes the server buffer up to 512 MB per connection before giving up.
- **Repro:** Read from the code, not run against the server.
- **Suggested fix:** Limit the header line length separately (e.g. reject if no CRLF in the first
  64 KB), and allow the buffer to exceed `MAX_BULK_LEN` by a small header margin.

### BUG-022: `GET` on a list key panics and poisons the store's mutex
- **Status:** Open
- **Severity:** High
- **Location:** `src/executor.rs` — `get`, `RedisDataType::List(_) => todo!()`
- **Problem:** The `todo!()` runs while the `MutexGuard` from `db.lock()` is still alive, so the
  panic poisons the mutex and every later `lock().unwrap()` panics too: one `GET` breaks the
  whole server.
- **Repro:** Verified 2026-10-14: `RPUSH k a`, then `GET k` → connection closed; after that every
  `SET`/`GET`/`RPUSH` from any client panics with `PoisonError` (`PING` still works, it doesn't lock).
- **Suggested fix:** Return Redis's `WRONGTYPE Operation against a key holding the wrong kind of value` error instead.

---

## Code quality / idiomatic Rust

### QUAL-010: Stored values can only be strings
- **Status:** Open
- **Location:** `src/data_manager.rs` — `RedisData`
- **Note:** Values are now `Bytes` (BUG-008 fixed), but later stages (lists, streams) need an enum
  of value types, e.g. `enum Value { String(Bytes), List(VecDeque<Bytes>) }`.
### QUAL-011: Unused dependencies
- **Status:** Open
- **Location:** `Cargo.toml`
- **Note:** `anyhow` and `thiserror` aren't used (`bytes` now is). Use them or remove them.
  `thiserror` would fit `ParserError` (it could then implement `std::error::Error`).

### QUAL-013: One `write_all` per pipelined reply
- **Status:** Open
- **Location:** `src/main.rs` — inner `while` loop
- **Note:** Each reply is written separately, so a pipeline of 100 commands makes 100 writes.
  Collect the encoded replies into one buffer and write once after the loop. Performance only.

### QUAL-015: Unclear names in the data model
- **Status:** Open
- **Location:** `src/data_manager.rs` — `RedisDataType`, `RedisData`, `insert_into_list`
- **Note:** `Byte` holds a whole string value (Redis calls it "string"); `insert_into_list` doesn't
  say which end, which matters once `LPUSH` exists; `RedisData` vs `RedisDataType` doesn't say which
  is the entry and which is the value (e.g. `Entry` / `Value`). Strings use `RedisData::new`, lists
  a struct literal; one constructor per kind would be consistent. Owner deferred this (2026-10-14).
---

## What's already good (keep doing this)

- `std::sync::Mutex` in async code is correct here, because the lock is never held across an `.await`.
- `Arc<Mutex<…>>` plus one spawned task per connection is the standard setup.
- The builder style in `RedisData::new(...).with_expiration(...)` is idiomatic.
- `Instant` (monotonic time) for expiry rather than wall-clock time.
- Multiple complete commands in one read are handled (basic pipelining).

---

## Resolved

### QUAL-014: `RPUSH` has no unit tests
- **Status:** Fixed (2026-10-14, uncommitted working tree; `cargo test` runs 50 tests, all passing)
- **Fix:** Executor tests for new key/append, `WRONGTYPE` on a string key, wrong argument counts and an expired string key (paused clock); `data_manager` tests for element order and `DataError::WrongType`. `DataError` now derives `Debug, PartialEq`.

### QUAL-009: Simplify the expiry check
- **Status:** Fixed (2026-10-14, uncommitted working tree; 44 unit tests and all 8 stages pass)
- **Fix:** Expiry check moved into `DataManager::remove_if_expired`, used by `get` and `insert_into_list`, written as `get(key).and_then(|res| res.expiration).is_some_and(|t| Instant::now() > t)`. Keeps `>` (Redis also treats a key as expired only strictly after its expiry time).

### BUG-023: `RPUSH` on a string key returns a non-Redis error
- **Status:** Fixed (2026-10-14, uncommitted working tree; verified against the running server: `SET s x`, `RPUSH s y` → `WRONGTYPE Operation against a key holding the wrong kind of value`)
- **Fix:** `insert_into_list` returns `Result<usize, DataError>`; the executor's `data_error` turns `DataError::WrongType` into the Redis message via `map_err`.

### BUG-008: `from_utf8_lossy` corrupts binary data
- **Status:** Fixed (2026-10-02, commit `08d7519`; 44 unit tests and all 7 stages pass, and `ECHO`/`SET`/`GET` with `\xff\xfe` and `a\x00b` were verified against the running server)
- **Fix:** `RespValue::BulkString` holds `Bytes` (copied raw with `Bytes::copy_from_slice`); `encode` appends the raw bytes with `extend_from_slice`; the store is `HashMap<Bytes, RedisData>` with `Bytes` values; the executor matches command names and options with `b"..."` patterns. `SimpleString`/`Error` stay `String` since they are always text.

### QUAL-012: No unit tests
- **Status:** Fixed (2026-10-01, uncommitted working tree; `cargo test` runs 40 tests, all passing)
- **Fix:** Unit tests in `parser.rs` (parsing, incomplete/invalid input, `encode`, round trip), `executor.rs` (every command, error replies, expiry with Tokio's paused clock) and `data_manager.rs`. Reintroducing the old `:` off-by-one or `*-1` bug makes tests fail. No end-to-end TCP tests yet (would need a `lib.rs` and a configurable port).

### BUG-011: Invalid or unsupported `SET` options are silently ignored
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** `set` reads options with an iterator: unknown options → `ERR syntax error`; missing, non-numeric or zero `EX`/`PX` values → `ERR invalid expire time in 'set' command`.

### BUG-012: `secs * 1000` can overflow
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** TTL is stored as a `Duration` (`Duration::from_secs` / `from_millis`), and `Instant::now().checked_add(ttl)` rejects values that would overflow.

### BUG-013: Missing arguments return the wrong error
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** Each command checks its argument count with a slice pattern and returns `ERR wrong number of arguments for '<cmd>' command`.

### BUG-019: `PING <message>` ignores its argument
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** `ping` replies with the message as a bulk string when one argument is given.

### QUAL-004: Encode responses in one place
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** Commands return `RespValue`; `RespValue::encode()` in `parser.rs` produces the RESP bytes, called once in `main.rs`.

### QUAL-005: `is_some()` followed by `unwrap_or(0)`
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** Replaced by `if let Some(ttl) = ttl`.

### QUAL-006: `#[allow(clippy::collapsible_if)]` hides a lint that could be fixed
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** Nesting removed with `let ... else`; the `allow` attribute is gone.

### QUAL-007: `to_uppercase()` allocates on every command
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** Uses `to_ascii_uppercase()`.

### QUAL-008: `execute` will keep growing as commands are added
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and the new error replies were verified against the running server)
- **Fix:** `execute` only converts arguments and dispatches; each command has its own function (`ping`, `echo`, `set`, `get`) returning `Result<RespValue, String>`.

### BUG-001: No buffering across socket reads
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and pipelined `PING PING` and a `PING` split across two writes were verified against the running server)
- **Fix:** `handle_connection` keeps a per-connection `BytesMut` (starts at 1 KB, grows as needed, capped at `parser::MAX_BULK_LEN`). After each read it parses and executes commands in a loop until `Incomplete`, so split and pipelined commands both work.

### BUG-002: `-ERR invalid request` branch is unreachable
- **Status:** Fixed (2026-10-01, uncommitted working tree; all 7 stages pass with `./test`, and pipelined `PING PING` and a `PING` split across two writes were verified against the running server)
- **Fix:** `ParserError::Invalid` now sends `-ERR invalid request` and closes the connection, as Redis does on a protocol error.

### BUG-003: Redundant `end` parameter on `parse`
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** `parse(input: &[u8])` takes only a slice; positions are relative to it. `main.rs` now calls `parse(&buffer)`.

### BUG-004: Parser has no bounds checks and panics on short input
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** `parse` returns `Result<(RespValue, usize), ParserError>`; missing CRLF or a short bulk body returns `ParserError::Incomplete` instead of panicking.

### BUG-005: `convert_byte_to_int` doesn't validate digits and overflows
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** `convert_byte_to_int` removed; `parse_int` uses `str::parse::<i64>()`. `$-1` → `Null`, `*-1` → `NullArray`, other negatives → `Invalid`, bulk length capped at 512 MB.

### BUG-006: Array parsing silently drops elements that fail to parse
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** Array elements are parsed with `?`, so any element error propagates.

### BUG-007: Trailing `\r\n` is never verified
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** Header lines end at the found CRLF; the bulk string branch verifies the trailing `\r\n` and returns `Invalid` otherwise.

### BUG-009: Fallback (inline) branch decodes bytes as Latin-1
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** Fallback branch removed; an unknown type byte now returns `Invalid("unexpected type byte: ..")`. Inline commands are not supported (the tester doesn't send them).

### BUG-010: `todo!()` for `:` and `+` lets clients crash the connection
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** `:` parses to `RespValue::Integer` and `+` to `RespValue::SimpleString`; no `todo!()` left.

### QUAL-001: Use `Result` instead of `Option` plus panics in the parser
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** Done together with BUG-004 (`ParserError { Incomplete, Invalid(String) }`).

### QUAL-002: Use slices and std parsing instead of index arithmetic
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** Uses `windows(2).position(..)` to find the line end, a `parse_int` helper and `let ... else`.

### QUAL-003: Rename `RedisRes` and remove the redundant length field
- **Status:** Fixed (2026-09-29, uncommitted working tree; verified by running the parser standalone on edge-case inputs. All 7 stages pass with `./test` as of 2026-10-01)
- **Fix:** Renamed to `RespValue`; length fields removed; `Null`/`NullArray`/`Integer`/`Error` variants added. `executor.rs` uses `RespValue`.

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

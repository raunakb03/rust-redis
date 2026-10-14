![progress-banner](https://codecrafters.io/landing/images/default_progress_banners/redis.png)

# rust-redis

A toy Redis server written in Rust for the
["Build Your Own Redis" Challenge](https://codecrafters.io/challenges/redis) on CodeCrafters.

## Status

All base stages up to and including **Expiry** pass, plus the first stage of the **Lists**
extension (*Create a list*).

Supported commands:

| Command | Notes |
|---|---|
| `PING [message]` | Replies `+PONG`, or the message as a bulk string if one is given |
| `ECHO <message>` | Replies with the message as a bulk string |
| `SET <key> <value> [EX seconds \| PX milliseconds]` | Other options (`NX`, `XX`, `GET`, `KEEPTTL`) are not supported yet and return `ERR syntax error` |
| `GET <key>` | Returns a null bulk string if the key is missing or expired. `GET` on a list key is not handled yet (BUG-022) |
| `RPUSH <key> <element>` | Appends to the list (creating it if needed) and replies with its new length. One element per call for now; a string key returns `WRONGTYPE` |

Multiple clients are served concurrently. Commands split across several reads and several
commands sent in one write (pipelining) are both handled. Wrong argument counts and invalid
`SET` options return Redis-style `-ERR` replies, and using a key with a command for the wrong
type returns `-WRONGTYPE`.

## Known issues

Known bugs and planned improvements are tracked in [`ISSUES.md`](ISSUES.md), each with a
severity, location and suggested fix. Keys and values are binary-safe. Values can be strings or lists. The main current limitations are that only `RPUSH` works on
lists so far (and `GET` on a list key crashes the server, see BUG-022), and expired keys are only
removed when they are read.

## Project layout

```
src/
├── main.rs          # TCP listener, one Tokio task per connection, read/respond loop
├── parser.rs        # RESP protocol parser and reply encoder
├── executor.rs      # Command dispatch, one function per command (PING, ECHO, SET, GET, RPUSH)
└── data_manager.rs  # In-memory store of string and list values, expiry checked on access
ISSUES.md            # Issue tracker shared between the owner and AI assistants
CLAUDE.md            # Instructions loaded automatically by Claude Code
```

## Running

Requires `cargo (1.96)`.

```sh
./your_program.sh          # build and start the server on 127.0.0.1:6379
redis-cli PING             # try it from another terminal
```

```sh
cargo clippy --all-targets # lint
cargo test                 # unit tests
./test [N]                 # run the first N CodeCrafters stages locally
codecrafters submit        # submit to CodeCrafters; test output streams to your terminal
```


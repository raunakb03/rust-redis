![progress-banner](https://codecrafters.io/landing/images/default_progress_banners/redis.png)

# rust-redis

A toy Redis server written in Rust for the
["Build Your Own Redis" Challenge](https://codecrafters.io/challenges/redis) on CodeCrafters.

## Status

All stages up to and including **Expiry** pass.

Supported commands:

| Command | Notes |
|---|---|
| `PING [message]` | Replies `+PONG`, or the message as a bulk string if one is given |
| `ECHO <message>` | Replies with the message as a bulk string |
| `SET <key> <value> [EX seconds \| PX milliseconds]` | Other options (`NX`, `XX`, `GET`, `KEEPTTL`) are not supported yet and return `ERR syntax error` |
| `GET <key>` | Returns a null bulk string if the key is missing or expired |

Multiple clients are served concurrently. Commands split across several reads and several
commands sent in one write (pipelining) are both handled. Wrong argument counts and invalid
`SET` options return Redis-style `-ERR` replies.

## Known issues

Known bugs and planned improvements are tracked in [`ISSUES.md`](ISSUES.md), each with a
severity, location and suggested fix. The most important current limitation is that values are
stored as UTF-8 `String`s, so binary data is corrupted (BUG-008). Expired keys are also only
removed when they are read.

## Project layout

```
src/
├── main.rs          # TCP listener, one Tokio task per connection, read/respond loop
├── parser.rs        # RESP protocol parser and reply encoder
├── executor.rs      # Command dispatch, one function per command (PING, ECHO, SET, GET)
└── data_manager.rs  # In-memory key-value store with expiry checked on read
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
./test [N]                 # run the first N CodeCrafters stages locally
codecrafters submit        # submit to CodeCrafters; test output streams to your terminal
```

## Working with AI assistants

`ISSUES.md` is the shared channel for reviews. An AI assistant working on this repo should
read it first, add any new issues it finds, and mark issues as fixed when the code resolves
them. The full rules are at the top of `ISSUES.md`. Claude Code picks this up automatically
through `CLAUDE.md`.

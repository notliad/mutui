# AGENTS.md

Guide for agents and contributors to `mutui`.

## Project Priorities

1. Code readability comes first.
2. Simple, predictable UX, and whenever it makes sense, vim-like.
3. Memory and resource optimization (the project’s main focus).

In case of conflicts between goals, prefer:

1. correctness and stability,
2. efficient memory/CPU usage,
3. simplicity of UX,
4. micro-optimized performance.

## Implementation Rules

### 1) Code Readability

* Write small functions with a single responsibility.
* Use explicit names for types, functions, and variables; avoid obscure abbreviations.
* Prefer simple, linear control flow; reduce `if`/`match` nesting when possible.
* Comment only non-obvious decisions (trade-offs, invariants, constraints).
* Avoid “magic”: named constants and clear types are better than scattered literals.
* Keep modules cohesive and boundaries clear between `mutui-common`, `mutui-daemon`, and `mutui-tui`.

### 2) Simple and Vim-like UX

* Prioritize keyboard interactions over adding new mouse actions.
* Whenever applicable, use vim-like conventions:

  * `j/k` to navigate lists,
  * `h/l` to move focus between columns/panels,
  * `gg/G` for start/end,
  * `/` for search,
  * `Esc` to go back/cancel.
* Keep feedback short and direct in the TUI (no visual noise).
* Preserve consistent behavior across screens: same key, same action.
* New shortcuts should have simple and discoverable fallbacks (help/tooltip).

### 3) Memory and Resources (Core Pillar)

* Avoid unnecessary allocations in hot paths.
* Prefer references (`&str`, slices) and buffer reuse when possible.
* Minimize cloning of `String`, `Vec`, and large structures.
* Avoid heavy work inside global locks (`Arc<Mutex<...>>`).
* Slow operations (I/O, search, library scanning, disk) must occur outside locks.
* Keep lock scope short to avoid freezing clients or degrading CPU usage.
* Do not introduce aggressive polling; prefer events, backoff, or batching.
* Handle external processes (`yt-dlp`, `mpv`, `pactl`) with timeouts, cleanup, and clear logging.

## Guidelines by Crate

### `mutui-daemon`

* Shared state must be protected with short-lived locks.
* Do not hold locks during network calls, subprocesses, or disk access.
* Preserve clean shutdown (resource release and audio module cleanup).

### `mutui-tui`

* UI updates must be lightweight and non-blocking for the input loop.
* Keep keyboard navigation as the primary experience.
* Rendering should avoid heavy recalculations per frame.

### `mutui-common`

* Protocol types should be stable, clear, and minimal.
* Avoid unnecessary growth of IPC payloads.

## PR Checklist

Before finishing, verify:

* The code is easier to read than before.
* The UX remains simple and consistent; new shortcuts are intuitive.
* No global lock is held during slow operations.
* No avoidable clones/allocations were introduced in frequent paths.
* Logs are useful for debugging without polluting output.
* Build and relevant tests pass locally.

## Don’ts

* Do not add architectural complexity without real need.
* Do not add heavy dependencies without strong justification.
* Do not sacrifice readability for premature micro-optimization.
* Do not introduce surprising or inconsistent UX behavior.

## Summary

When in doubt, choose the solution that is easiest to maintain, with the lowest memory/CPU cost and the best keyboard-first user experience.

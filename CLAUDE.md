# Winter War 2039: development guide

An isometric 2D game in Rust on Bevy 0.19 (nightly toolchain, edition 2024).
`src/main.rs` has a module-level overview of the architecture; read it first.

## Architecture rules

- One concern per module, each exposed as a Bevy `Plugin` and registered in `main.rs`.
- `GridPos` is the source of truth for position. Only `iso.rs` writes the `Transform` of
  grid entities. Gameplay code changes `GridPos`; never set a sprite's `Transform` by hand.
- Systems that change `GridPos` must run `.before(ProjectSet)`.
- Depth is y-sorted (see `Z_PER_PIXEL` in `iso.rs`). `ScreenOffset::z` biases must stay
  smaller than one row step (0.001).
- Anything that needs the *contents* of an asset spawns in `OnEnter(AppState::Running)`.
  Systems that don't need asset data may use `Startup`.
- Art is exported from Aseprite as `assets/<name>.png` + `assets/<name>.json`. To add a
  sheet, add its name to `SHEET_NAMES` in `sheets.rs`.
- Prefer handles/IDs over string lookups in per-frame code. Use change detection
  (`Changed<T>`, `set_if_neq`) so static things cost nothing.

## Literate programming (always)

The code must read like an explanation of the program, not a list of statements.

- Every file starts with a `//!` header: what the file is for, why it exists, and how it
  fits the data flow. A reader should understand the module before reading any code.
- Every item (struct, component, system, constant) gets a `///` comment saying *what it is
  and why it exists*. Explain the reasoning, assumptions and non-obvious maths (derive
  formulas in the comment), not just what the line does.
- Multi-step functions are narrated in order with inline `//` comments ("Step 1: ...").
- Comments are prose, written for a reader new to the project. No comments that restate
  the code.
- **Comments and code change together.** When behavior changes, update the surrounding
  explanation in the same commit. A stale comment is a bug.
- Put the module overview in the file header, so `cargo doc --open` renders as a guide.
  Link modules with `[`mod@name`]` where the name is ambiguous (e.g. `unit`).

## Git workflow

### Branches
- `main` always builds. **Never commit new work directly to `main`.**
- Every new feature, fix or refactor goes on its own branch, named
  `type/short-description` in kebab-case, where `type` is a conventional commit type:
  `feat/unit-movement`, `fix/cursor-offset`, `refactor/iso-module`.
- Create the branch before making the first change.

### Commits
[Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`.

- Types: `feat`, `fix`, `refactor`, `perf`, `docs`, `test`, `build`, `chore`, `style`.
- Scope is the module when useful: `feat(unit): ...`, `fix(iso): ...`.
- Imperative, lower-case summary, no trailing period, at most ~72 characters.
- Use the body to explain *why* when the summary isn't enough. Mark breaking changes with
  `!` and a `BREAKING CHANGE:` footer.
- Commit small and often on a branch: the branch's history is working history, because it
  gets squashed. Each commit still needs a valid conventional message.
- End commits with the attribution trailer the harness specifies.

### Merging
- Features land in `main` via a **pull request with squash merge**. The squash commit
  title is itself a conventional commit and summarizes the whole branch.
- Open PRs with `gh pr create`; the description says what changed and why, and ends with
  the harness's PR attribution line.
- **Never merge into `main`, push `main`, or force-push unless explicitly asked.**
  Committing on a feature branch is fine without asking; pushing a branch and opening a
  PR is fine when work is ready, but merging is the user's call.
- Note: this repo currently has no `origin` remote. Ask the user to add one before trying
  to push or open a PR.

## Before declaring work done

Run all of these and fix what they report; do not leave new warnings behind:

1. `cargo fmt`
2. `cargo clippy` (no warnings)
3. `cargo check`
4. `cargo test`

Add unit tests for pure logic (the projection and its inverse in `iso.rs`, tag
`sequence` expansion in `sheets.rs`). If a change affects runtime behavior, also run the
game briefly (`cargo run`) and say plainly that you did or did not, since the tests
cannot judge how it looks.

## Build notes

- Dev builds use nightly, `clang` + `mold`, and the Cranelift backend for this crate (LLVM
  for dependencies). Config lives in `.cargo/config.toml` and `rust-toolchain.toml`;
  don't change it without a reason.
- Bevy's `dynamic_linking` is on for fast rebuilds. Disable it for release builds.
- The sheet JSON hot-reloads only if Bevy's `file_watcher` feature is enabled.

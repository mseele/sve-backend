## Context

The SVE backend has 8,674 lines of Rust across 18 files but only 16 test functions in 5 files. The largest untested areas are `db.rs` (2023 lines), `api.rs` (805 lines), `logic/export.rs` (834 lines), `logic/events.rs` (partially tested), and logic modules for membership, contact, and news. Current tests cover pure business logic (CSV parsing, price calculation, template rendering) that doesn't require external dependencies.

Functions that depend on `PgPool`, email sending via SMTP, or AWS Secrets Manager are currently untestable. The project uses `sqlx` 0.8.6, `lettre`, and `axum` - no mocking framework is present.

## Goals / Non-Goals

**Goals:**
- Make all pure business logic testable with zero mocking overhead
- Introduce trait abstractions for email dependencies to enable unit testing of logic modules
- Use `#[sqlx::test]` for database integration tests (isolated DB per test, automatic migration + cleanup)
- Use `mockall` for generating mock implementations of traits (zero boilerplate)
- Achieve test coverage for: models (enums, conversions), db.rs (via `#[sqlx::test]`), logic modules (news, membership, contact, events), and email infrastructure
- Follow existing code conventions (inline `#[cfg(test)]` modules, `pretty_assertions`, `anyhow::Result`)

**Non-Goals:**
- Testing `api.rs` HTTP handlers (requires full Axum test harness - separate concern)
- Testing `logic/calendar.rs` (already has integration tests hitting live Google Calendar API)
- Testing `logic/export.rs` PDF/Excel generation (complex output, low ROI for unit tests)

## Decisions

### 1. mockall for trait mocking

**Choice:** Use `mockall` crate with `#[automock]` attribute on traits.

**Rationale:**
- `mockall` is the most mature Rust mocking framework (1.8k+ GitHub stars)
- `#[automock]` generates mock structs with expectation verification for free (`.once()`, `.times()`, argument matchers)
- The project already uses proc-macros (`serde`, `sqlx`) so compile-time overhead is relative
- Eliminates all hand-written mock boilerplate - focus on test logic, not mock setup
- Supports async traits natively (put `#[async_trait]` before `#[automock]`)

**Alternatives considered:**
- Hand-written mocks: rejected - significant boilerplate per trait method, no expectation verification
- `faux`: rejected - mocks struct methods via `unsafe`, less expectation verification than mockall
- `#[cfg(test)]` overrides: rejected - test code diverges from production paths

### 2. #[sqlx::test] for database integration tests

**Choice:** Use sqlx's built-in `#[sqlx::test]` macro for testing `db.rs` and logic modules against real PostgreSQL.

**Rationale:**
- Already available as a dependency (sqlx 0.8.6) - no new dependencies needed
- Creates isolated database per test using Postgres sequences (no collision between parallel tests)
- Automatically applies migrations from `./migrations/`
- Supports SQL fixtures for seed data
- Cleans up databases after successful tests; keeps them on failure for debugging
- Eliminates the need for a `DbOps` trait abstraction - test real SQL directly

**How it works:**
```rust
#[sqlx::test]
async fn test_subscribe(pool: PgPool) {
    // fresh DB with migrations applied, injected as parameter
    let result = db::subscribe(&pool, subscription).await;
    assert!(result.is_ok());
}
```

**Alternatives considered:**
- Mocking `db.rs` via trait: rejected - SQL bugs wouldn't be caught; real DB testing is more valuable
- `testcontainers`: adds Docker dependency; `#[sqlx::test]` is simpler
- Manual DB setup per test: verbose, error-prone, no automatic cleanup

### 3. Single EmailSender trait

**Choice:** One `EmailSender` trait with `#[automock]` for email operations.

**Rationale:**
- `email.rs` has 4 functions (`test_connection`, `send_message`, `send_messages`, `get_account_by_type/address`) - group into one trait
- `mockall` generates the mock struct automatically - no manual implementation needed
- Logic modules that send email use `&impl EmailSender` parameters for zero-cost abstraction in production

### 4. Production implementation via wrapper struct

**Choice:** `RealEmailSender` struct that implements the trait by delegating to existing free functions.

**Rationale:**
- Minimal changes to existing code - existing free functions stay as-is
- Wrapper struct just calls through to the real implementations
- Logic modules receive `&impl EmailSender` (monomorphic) or `&dyn EmailSender` (dynamic dispatch)

### 5. Extract pure functions from mixed-dependency functions

**Choice:** Where possible, extract pure logic (HTML building, body formatting) into standalone functions that are independently testable.

**Examples:**
- `membership.rs::create_internal_email` → extract HTML body builder as pure function
- `contact.rs::message` → extract body formatting as pure function
- `news.rs::send_mail` → extract email body construction as pure function

## Risks / Trade-offs

- **mockall compile time:** mockall adds `syn`, `quote`, `proc-macro2` dependencies. Compile time increases slightly, but only for test builds (`dev-dependencies`).
- **`#[sqlx::test]` requires running PostgreSQL:** Tests need a local Postgres instance. CI must provide one. This is acceptable since the app already requires Postgres.
- **EmailSender trait maintenance:** One trait needs updating when email interfaces change. Mitigated by small trait surface (4 methods).
- **Refactoring scope:** Logic modules need signature changes to accept `&impl EmailSender`. This is a one-time cost affecting ~4 files.
- **No `DbOps` trait needed:** Using `#[sqlx::test]` directly against `db.rs` means we skip the trait abstraction layer for DB - simpler design, better test coverage.

## Migration Plan

1. Add `mockall` to `[dev-dependencies]` in `Cargo.toml`
2. Create `EmailSender` trait with `#[automock]` in `src/email.rs`
3. Create `RealEmailSender` wrapper struct
4. Update logic modules (`news.rs`, `membership.rs`, `contact.rs`, `events.rs`) to accept `&impl EmailSender`
5. Update call sites in `src/api.rs` and `src/logic/tasks.rs` to pass `RealEmailSender`
6. Write pure logic tests first (models.rs, enum conversions) - no trait changes needed
7. Write `#[sqlx::test]` integration tests for `db.rs` core functions
8. Write mockall-based unit tests for each logic module
9. Run `cargo test` and `cargo clippy` after each module
10. No deployment changes - all refactoring is internal

## Open Questions

- Which `db.rs` functions to prioritize for `#[sqlx::test]` coverage? (Recommend: subscribe/unsubscribe, get_event, get_bookings, write_event)
- Should `EmailSender` use `&self` or associated functions? (Recommend: `&self` for mockability)

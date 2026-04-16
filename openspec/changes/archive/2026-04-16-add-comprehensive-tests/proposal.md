## Why

The SVE backend has 16 test functions across 5 of 18 source files. The two largest files (`db.rs` at 2023 lines, `logic/export.rs` at 834 lines) have zero tests. The HTTP layer (`api.rs` at 805 lines) and most logic modules (`membership.rs`, `contact.rs`, `news.rs`, `tasks.rs`) are completely untested. Pure business logic like enum conversions, capacity checks, and currency formatting is easy to test today but has no coverage. Functions with external dependencies (DB, email) are untestable without introducing trait-based abstractions.

## What Changes

- Introduce `EmailSender` trait with `mockall` `#[automock]` for email mocking in tests
- Use `#[sqlx::test]` for database integration tests against real PostgreSQL (no DbOps trait needed)
- Add pure logic tests for all models, enums, and extractable functions (no mocking needed)
- Add mocked unit tests for `logic/news.rs`, `logic/membership.rs`, `logic/contact.rs`, and `logic/events.rs`
- Add `#[sqlx::test]` integration tests for `db.rs` core functions
- Extract testable pure logic from mixed-dependency functions (e.g., HTML body building in `membership.rs`)

## Capabilities

### New Capabilities
- `trait-abstractions`: Introduce `EmailSender` trait with `mockall` `#[automock]`, production wrapper, and logic module refactoring
- `model-tests`: Unit tests for `MembershipType`, `EventCounter`, `LifecycleStatus`, `EventType`, `NewsTopic`, `ToEuro`/`FromEuro`, `EventId` serialization
- `logic-unit-tests`: Unit tests for pure functions in `events.rs`, `news.rs`, `membership.rs`, `contact.rs` using mockall mocks and `#[sqlx::test]` for DB operations

### Modified Capabilities
None - this adds test coverage without changing existing requirements.

## Impact

- `src/email.rs`: Extract `EmailSender` trait with `#[automock]`, create `RealEmailSender` wrapper
- `src/db.rs`: Test directly with `#[sqlx::test]` - no trait abstraction needed
- `src/logic/news.rs`: Refactor to accept `&impl EmailSender`
- `src/logic/membership.rs`: Refactor to accept `&impl EmailSender`, extract pure HTML builder
- `src/logic/contact.rs`: Refactor to accept `&impl EmailSender`
- `src/models.rs`: Add test module for enums and pure methods
- `Cargo.toml`: Add `mockall` to `[dev-dependencies]`
- No changes to public API or behavior - pure internal refactoring

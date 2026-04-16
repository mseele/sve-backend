## ADDED Requirements

### Requirement: EmailSender trait with mockall
The system SHALL provide an `EmailSender` trait annotated with `#[automock]` from the `mockall` crate, abstracting email sending operations for testability.

#### Scenario: Trait definition with automock
- **WHEN** the `EmailSender` trait is defined with `#[automock]`
- **THEN** `mockall` generates a `MockEmailSender` struct with expectation methods for test use

#### Scenario: Trait methods
- **WHEN** the trait is defined
- **THEN** it exposes async methods: `test_connection`, `send_message`, `send_messages`, `get_account_by_type`, `get_account_by_address`

#### Scenario: Production implementation
- **WHEN** the `RealEmailSender` struct implements `EmailSender`
- **THEN** it delegates to the existing `email::` free functions

#### Scenario: Test mock setup
- **WHEN** a test creates a `MockEmailSender`
- **THEN** it can set expectations on method calls (`.once()`, `.times(N)`, argument matchers) and configure return values

### Requirement: Logic modules accept EmailSender trait
The system SHALL refactor logic modules (`news.rs`, `membership.rs`, `contact.rs`) to accept `&impl EmailSender` instead of calling `email::` free functions directly.

#### Scenario: Function signatures use trait bounds
- **WHEN** a logic function needs to send email
- **THEN** it accepts `&impl EmailSender` as a parameter

#### Scenario: Existing call sites updated
- **WHEN** `api.rs` or `tasks.rs` calls a logic function that sends email
- **THEN** it passes a `RealEmailSender` instance

### Requirement: Database tests use #[sqlx::test]
The system SHALL use sqlx's built-in `#[sqlx::test]` macro for testing database operations against real PostgreSQL.

#### Scenario: Isolated database per test
- **WHEN** a test function is annotated with `#[sqlx::test]` and accepts `PgPool`
- **THEN** sqlx creates a fresh database, applies migrations, and injects the pool

#### Scenario: Automatic cleanup
- **WHEN** a `#[sqlx::test]` test completes successfully
- **THEN** the test database is automatically dropped
- **WHEN** a `#[sqlx::test]` test fails
- **THEN** the test database is preserved for debugging

#### Scenario: Fixture support
- **WHEN** a test is annotated with `#[sqlx::test(fixtures(...))]`
- **THEN** SQL fixture files are executed before the test runs to seed data

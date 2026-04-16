## 1. Setup

- [x] 1.1 Add `mockall` to `[dev-dependencies]` in `Cargo.toml`

## 2. EmailSender Trait

- [x] 2.1 Define `EmailSender` async trait in `src/email.rs` with `#[automock]`: methods `test_connection`, `send_message`, `send_messages`, `get_account_by_type`, `get_account_by_address`
- [x] 2.2 Create `RealEmailSender` struct implementing `EmailSender` by delegating to existing free functions
- [x] 2.3 Update `news.rs`, `membership.rs`, `contact.rs` to accept `&impl EmailSender`
- [x] 2.4 Refactor `events.rs` functions to accept `&impl EmailSender` (8 remaining free-function email calls):
  - [x] 2.4.1 `update` (line 122: `email::send_messages`)
  - [x] 2.4.2 `cancel_booking` (line 274: `email::send_messages`)
  - [x] 2.4.3 `send_event_email` (line 348: `email::send_messages`)
  - [x] 2.4.4 `send_event_reminders` (line 397: `email::send_messages`)
  - [x] 2.4.5 `send_payment_reminders` (lines 419, 461: `email::get_account_by_type` + `email::send_messages`)
  - [x] 2.4.6 `send_booking_mail` (line 737: `email::send_message`)
  - [x] 2.4.7 `send_participation_confirmation` (line 1059: `email::send_messages`)
- [x] 2.5 Update call sites in `src/api.rs` and `src/logic/tasks.rs` to pass `RealEmailSender`

## 3. Model Tests (pure logic, no mocking)

- [x] 3.1 Add tests for `MembershipType::get_label()` covering all 7 variants
- [x] 3.2 Add tests for `MembershipType::get_department()` covering Fitness vs Hauptverein
- [x] 3.3 Add tests for `EventCounter::is_booked_up()` covering unlimited (-1), full, partial, and empty states
- [x] 3.4 Add tests for `LifecycleStatus::is_bookable()` covering all 7 variants
- [x] 3.5 Add tests for `EventType::subject_prefix()` for Fitness and Events
- [x] 3.6 Add tests for `EventType::FromStr` with valid and invalid inputs
- [x] 3.7 Add tests for `LifecycleStatus::FromStr` with valid and invalid inputs (case-insensitive)
- [x] 3.8 Add tests for `NewsTopic::display_name()` and `NewsTopic::FromStr` for all variants
- [x] 3.9 Add tests for `ToEuro` formatting with rounding edge cases
- [x] 3.10 Add tests for `FromEuro` parsing with German decimal format

## 4. Database Integration Tests (#[sqlx::test])

- [x] 4.1 Add `#[sqlx::test]` tests for `db::subscribe` and `db::unsubscribe`
- [x] 4.2 Add `#[sqlx::test]` tests for `db::get_subscriptions`
- [x] 4.3 Add `#[sqlx::test]` tests for `db::write_event` and `db::get_event`
- [x] 4.4 Add `#[sqlx::test]` tests for `db::get_bookings` and `db::cancel_event_booking`
- [x] 4.5 Add `#[sqlx::test]` tests for `db::mark_as_payed` and `db::update_payment`

## 5. Logic Module Unit Tests (mockall)

- [x] 5.1 Add tests for `news::subscribe()` with mocked email (verify confirmation email sent)
- [x] 5.2 Add tests for `news::send_mail` body construction (single topic, multiple topics)
- [x] 5.3 Add tests for `membership::build_internal_email_html()` (pure function: basic + family variants)
- [x] 5.4 Add tests for `membership::application()` with mocked email (newsletter flag, welcome + internal emails)
- [x] 5.5 Add tests for `contact::build_contact_body()` (pure function: all fields, missing phone, trimming)
- [x] 5.6 Add tests for `contact::emails()` grouping by EmailType with mocked email sender
- [x] 5.7 Add tests for `contact::message()` with MockEmailSender (verify email sent to correct account)
- [x] 5.8 Add tests for `events::send_event_email` validation (bookings/waiting list combinations)
- [x] 5.9 Add tests for `events::create_prebooking_link` with both event types
- [x] 5.10 Add tests for `news::unsubscribe()` via `#[sqlx::test]`
- [x] 5.11 Add tests for `news::get_subscriptions()` via `#[sqlx::test]`
- [x] 5.12 Add tests for `tasks::check_email_connectivity()` with MockEmailSender
- [x] 5.13 Add tests for `events::booking()` via `#[sqlx::test]` + MockEmailSender (requires task 2.4)
- [x] 5.14 Add tests for `events::prebooking()` via `#[sqlx::test]` + MockEmailSender (requires task 2.4)
- [x] 5.15 Add tests for `events::cancel_booking()` via `#[sqlx::test]` + MockEmailSender (requires task 2.4.2)
- [x] 5.16 Add tests for `events::send_booking_mail()` with MockEmailSender (requires task 2.4.6)
- [x] 5.17 Add tests for `events::update()` via `#[sqlx::test]` + MockEmailSender (requires task 2.4.1)
- [x] 5.18 Add tests for `events::send_event_reminders()` via `#[sqlx::test]` + MockEmailSender (requires task 2.4.4)
- [x] 5.19 Add tests for `events::send_payment_reminders()` via `#[sqlx::test]` + MockEmailSender (requires task 2.4.5)
- [x] 5.20 Add tests for `events::send_participation_confirmation()` via `#[sqlx::test]` + MockEmailSender (requires task 2.4.7)

## 6. Verification

- [x] 6.1 Run `cargo test` and ensure all tests pass (57 tests passing)
- [x] 6.2 Run `cargo clippy` and fix any warnings (only pre-existing `dead_code` warning)
- [x] 6.3 Run `cargo fmt -- --check` to verify formatting

## Dependency Graph

Tasks 5.13-5.20 (events.rs tests) depend on corresponding 2.4.x tasks (events.rs EmailSender refactoring). Complete the refactor first, then write tests.

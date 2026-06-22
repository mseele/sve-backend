# Agent Guidelines for SVE Backend

Rust backend service for the SVE website, deployed as an AWS Lambda.

## Commands

```bash
# Build / test / lint
cargo build
cargo test
cargo clippy --all-features
cargo fmt

# Lambda CI build
cargo build --release --locked --target aarch64-unknown-linux-musl

# Update SQLx offline metadata after query changes
cargo sqlx prepare -- --all-targets
```

## Style

- Import order: `std::`, external crates, `crate::` (blank lines between groups).
- `pub(crate)` for crate-wide items.
- PascalCase types/enums, snake_case functions/variables, SCREAMING_SNAKE_CASE constants.
- `anyhow::Result<T>`, `?`, `bail!`, `.context()` for errors.
- `i32` for DB IDs, `BigDecimal` for money, `DateTime<Utc>` for timestamps.
- Compile-time checked SQLx queries; `QueryBuilder` for dynamic queries.
- Inline `#[cfg(test)]` modules; use `pretty_assertions`.

## Structure

- Logic: `src/logic/`
- Models: `src/models.rs`
- Routes: `src/api.rs`
- DB: `src/db.rs`

## Environment

Load from `.env` in development:

- `DATABASE_URL`
- `CAPTCHA_SECRET`
- Secrets managed via `src/logic/secrets.rs`

Run local server: `cargo run` (port 8080).

## Agent skills

### Issue tracker

Issues live in GitHub Issues (`mseele/sve-backend`). External PRs are not a triage surface. See `docs/agents/issue-tracker.md`.

### Triage labels

Use the default canonical label strings (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context repo: read `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.

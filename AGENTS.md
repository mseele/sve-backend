# Agent Guidelines for SVE Backend

This is a Rust backend service for the SVE website, deployed as an AWS Lambda.

## Build Commands

```bash
# Build for development
cargo build

# Build for release (optimized)
cargo build --release

# Build for Lambda target (CI)
cargo build --release --locked --target aarch64-unknown-linux-musl
```

## Test Commands

```bash
# Run all tests
cargo test

# Run a single test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

## Lint Commands

```bash
# Check code with Clippy
cargo clippy

# Check with all features
cargo clippy --all-features

# Format code
cargo fmt

# Check formatting without modifying
cargo fmt -- --check
```

## Code Style Guidelines

### Imports

Order imports in three groups separated by blank lines:
1. Standard library (`std::`)
2. External crates (`axum::`, `sqlx::`, etc.)
3. Internal modules (`crate::`)

Example:
```rust
use std::collections::HashMap;

use axum::extract::State;
use sqlx::PgPool;

use crate::models::Event;
use crate::db;
```

### Visibility

Use `pub(crate)` for items that need to be accessible across the crate but not externally.

### Naming Conventions

- **Structs/Enums**: PascalCase (`EventBooking`, `LifecycleStatus`)
- **Functions/Variables**: snake_case (`get_events`, `event_id`)
- **Constants**: SCREAMING_SNAKE_CASE (`MESSAGE_FAIL`)
- **Enum variants**: PascalCase (`Draft`, `Published`)

### Types

- Use `anyhow::Result<T>` for error handling
- Use `i32` for database IDs (wrapped in newtypes like `EventId`)
- Use `BigDecimal` for monetary values (from `bigdecimal` crate)
- Use `DateTime<Utc>` for timestamps
- Newtype pattern for type-safe IDs with custom serialization

### Error Handling

- Use `anyhow` for error propagation with `?` operator
- Use `bail!()` macro for early returns with errors
- Use `.context()` for adding error context
- Custom error types implement `std::error::Error`

Example:
```rust
use anyhow::{Result, bail, Context};

pub(crate) async fn fetch_data() -> Result<Data> {
    let result = db::query()
        .await
        .context("Failed to fetch data from database")?;
    
    if result.is_empty() {
        bail!("No data found");
    }
    
    Ok(result)
}
```

### SQLx Patterns

- Use compile-time checked queries (stored in `.sqlx/`)
- Use `QueryBuilder` for dynamic queries
- Run `cargo sqlx prepare` to update query metadata after SQL changes

### Async/Await

- All database and external service calls are async
- Use `tokio` runtime with `#[tokio::main]`
- Prefer `async fn` for most operations

### Testing

- Write inline tests in `#[cfg(test)]` modules at file end
- Use `pretty_assertions` crate for better test output
- Use descriptive test names: `test_price_calculation`

Example:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_price() {
        let booking = EventBooking::new(...);
        assert_eq!(booking.price(), expected);
    }
}
```

### Documentation

- Document complex business logic with inline comments
- Document public APIs with doc comments (`///`)
- Include doc comments on enums explaining variants

### Code Organization

- Keep modules focused on single responsibility
- Logic lives in `src/logic/`, models in `src/models.rs`
- Route handlers in `src/api.rs`
- Database operations in `src/db.rs`

### Dependencies

Before adding new dependencies, check if existing ones can handle the use case:
- Serialization: `serde`
- HTTP: `axum`, `tower`, `hyper`
- Database: `sqlx`
- Email: `lettre`
- Templates: `handlebars`
- PDF: `printpdf`
- Excel: `simple_excel_writer`
- Dates: `chrono`, `chrono-tz`
- Validation: `iban_validate`, `hcaptcha`

## Environment Setup

The application requires environment variables (loaded from `.env` in development):
- `DATABASE_URL`: PostgreSQL connection string
- `CAPTCHA_SECRET`: hCaptcha secret key
- Various secrets managed via `src/logic/secrets.rs`

Run with `cargo run` for local development server on port 8080.

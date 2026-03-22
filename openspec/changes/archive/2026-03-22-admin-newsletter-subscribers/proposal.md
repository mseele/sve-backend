## Why

The current newsletter subscriber list endpoint (`GET /api/news/subscribers`) is publicly accessible without authentication. This exposes subscriber email addresses - a privacy concern. Admins need access to subscriber data for managing newsletters, but this should be behind OAuth like all other admin endpoints.

## What Changes

- Add a new OAuth-protected admin endpoint `GET /api/admin/news/subscribers` that returns newsletter subscribers as structured JSON (grouped by topic)
- The new endpoint reuses the existing `news::get_subscriptions` logic
- **Future step** (separate change): Remove the public `GET /api/news/subscribers` endpoint once the admin UI has been updated to use the new endpoint

## Capabilities

### New Capabilities
- `admin-news-subscribers`: OAuth-protected endpoint to retrieve newsletter subscribers grouped by topic, returning JSON instead of HTML

### Modified Capabilities
- None - the public endpoint removal is a separate step after admin UI integration

## Impact

- `src/api.rs`: Add new admin route and handler, later remove public route and handler
- `src/logic/news.rs`: No changes needed - reuse existing `get_subscriptions` function
- `src/db.rs`: No changes needed
- `src/models.rs`: No changes needed

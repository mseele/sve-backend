## Context

The newsletter subscriber endpoint (`GET /api/news/subscribers`) currently returns an HTML page with subscriber emails grouped by topic. This endpoint is publicly accessible without authentication, which exposes subscriber email addresses - a privacy concern. Admins need access to this data, but it should be behind OAuth like all other admin endpoints.

The admin UI will eventually consume this data, replacing the current raw HTML view. Once the admin UI is updated, the public endpoint should be removed.

## Goals / Non-Goals

**Goals:**
- Provide OAuth-protected access to newsletter subscriber data
- Return structured JSON instead of HTML for programmatic consumption
- Follow existing admin endpoint patterns for consistency

**Non-Goals:**
- Adding/modifying/deleting subscribers (read-only for now)
- Changing the subscription model or database schema
- Updating the admin UI (separate follow-up task)

## Decisions

**JSON format**: Return subscribers as a JSON array of objects, each containing the email and subscribed topics. This is consistent with other admin endpoints (e.g., `admin_events`) which return `Json(...)`.

**Reuse existing logic**: The `news::get_subscriptions` function already fetches and groups subscribers by topic. The new endpoint will reuse this logic and transform the `HashMap<NewsTopic, HashSet<String>>` into a structured JSON response.

**Route structure**: Place under `/api/admin/news/subscribers` to follow the existing admin route nesting pattern (similar to `/api/admin/events/`, `/api/admin/contact/`).

## Risks / Trade-offs

- **Security**: The public endpoint remains until admin UI is updated - subscriber data continues to be exposed until then. → Mitigation: Remove public endpoint promptly after admin UI integration.
- **Breaking change**: Removing the public endpoint will break any existing consumers. → Mitigation: Admin UI is the only known consumer; coordinate removal with UI update.

## ADDED Requirements

### Requirement: Admin can retrieve newsletter subscribers
The system SHALL provide an OAuth-protected endpoint that returns newsletter subscribers grouped by topic as structured JSON.

#### Scenario: Authenticated admin retrieves subscribers
- **WHEN** an authenticated admin user sends a GET request to `/api/admin/news/subscribers`
- **THEN** the system returns a JSON object with topics as keys and arrays of subscriber emails as values

#### Scenario: Unauthenticated request is rejected
- **WHEN** an unauthenticated user sends a GET request to `/api/admin/news/subscribers`
- **THEN** the system returns a 401 Unauthorized response

#### Scenario: No subscribers returns empty groups
- **WHEN** there are no newsletter subscribers in the system
- **THEN** the system returns an empty JSON object with all topic keys present but empty arrays

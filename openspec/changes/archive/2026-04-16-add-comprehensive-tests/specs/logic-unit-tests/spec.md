## ADDED Requirements

### Requirement: News module email body construction
The system SHALL test the email body construction logic in `news.rs::send_mail` using mocked email sender.

#### Scenario: single topic subscription email
- **WHEN** a subscription has exactly one topic
- **THEN** the email body contains topic-specific content (subject, regards, kind text)

#### Scenario: multiple topic subscription email
- **WHEN** a subscription has multiple topics
- **THEN** the email body lists all topics and uses General defaults for subject/regards

### Requirement: News module subscribe/unsubscribe flow
The system SHALL test `subscribe()` and `unsubscribe()` using mocked DB.

#### Scenario: subscribe sends confirmation email
- **WHEN** `subscribe()` is called
- **THEN** the DB subscribe method is called AND a confirmation email is sent

#### Scenario: subscribe_to_news with send_email=false
- **WHEN** `subscribe_to_news()` is called with `send_email=false`
- **THEN** the DB subscribe method is called AND no email is sent

### Requirement: Membership module internal email HTML
The system SHALL test the HTML body generation in `create_internal_email`.

#### Scenario: basic membership application HTML
- **WHEN** creating an internal email for a standard application
- **THEN** the HTML contains salutation, name, address, IBAN, bank name, and BIC

#### Scenario: family membership includes family members table
- **WHEN** creating an internal email for a family application with family members
- **THEN** the HTML includes a "Familienmitglieder" section with first name, last name, birthday columns

#### Scenario: non-family membership omits family section
- **WHEN** creating an internal email for a non-family application
- **THEN** the HTML does not contain "Familienmitglieder" section

### Requirement: Membership module application flow
The system SHALL test `application()` using mocked dependencies.

#### Scenario: application subscribes to newsletter when selected
- **WHEN** `application()` is called with `newsletter=true`
- **THEN** `subscribe_to_news` is called with the applicant's email and `send_email=false`

#### Scenario: application skips newsletter when not selected
- **WHEN** `application()` is called with `newsletter=false`
- **THEN** no newsletter subscription is made

#### Scenario: application sends welcome and internal emails
- **WHEN** `application()` is called
- **THEN** a welcome email is sent to the applicant AND an internal email with CSV attachment is sent to mitglieder@sv-eutingen.de

### Requirement: Contact module body formatting
The system SHALL test the message body construction in `contact.rs::message`.

#### Scenario: message body includes all fields
- **WHEN** formatting a contact message
- **THEN** the body contains name, email, phone (if present), and message text

#### Scenario: message body omits phone when empty
- **WHEN** formatting a contact message with `phone=None` or empty phone
- **THEN** the body does not contain "Telefon:" line

#### Scenario: message body trims whitespace
- **WHEN** formatting a contact message with whitespace in fields
- **THEN** trimmed values are used in the body

### Requirement: Contact module email grouping
The system SHALL test `emails()` grouping by email type using mocked dependencies.

#### Scenario: emails grouped by type
- **WHEN** `emails()` is called with multiple emails of different types
- **THEN** emails are grouped by `EmailType` and sent in batches per type

### Requirement: Events module prebooking link tests
The system SHALL test `create_prebooking_link` with additional edge cases.

#### Scenario: Fitness event link
- **WHEN** creating a link for `EventType::Fitness`
- **THEN** the URL starts with `https://www.sv-eutingen.de/fitness?code=`

#### Scenario: Events event link
- **WHEN** creating a link for `EventType::Events`
- **THEN** the URL starts with `https://www.sv-eutingen.de/events?code=`

### Requirement: Events module send_event_email validation
The system SHALL test validation logic in `send_event_email`.

#### Scenario: error when neither bookings nor waiting list selected
- **WHEN** `send_event_email()` is called with `bookings=false` and `waiting_list=false`
- **THEN** an error is returned

#### Scenario: enrolled filter for bookings only
- **WHEN** `bookings=true` and `waiting_list=false`
- **THEN** `get_bookings` is called with `enrolled=Some(true)`

#### Scenario: enrolled filter for waiting list only
- **WHEN** `bookings=false` and `waiting_list=true`
- **THEN** `get_bookings` is called with `enrolled=Some(false)`

#### Scenario: no filter when both selected
- **WHEN** `bookings=true` and `waiting_list=true`
- **THEN** `get_bookings` is called with `enrolled=None`

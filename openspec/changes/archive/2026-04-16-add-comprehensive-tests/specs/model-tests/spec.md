## ADDED Requirements

### Requirement: MembershipType enum tests
The system SHALL test all `MembershipType` enum methods with every variant.

#### Scenario: get_label returns correct label for each variant
- **WHEN** `get_label()` is called on each `MembershipType` variant
- **THEN** it returns the correct German label string (e.g., `Fitness` → "Sparte Fitness", `Family` → "Familienbeitrag beliebige Kinder")

#### Scenario: get_department returns correct department
- **WHEN** `get_department()` is called on `Fitness`
- **THEN** it returns "Fitness"
- **WHEN** `get_department()` is called on any other variant
- **THEN** it returns "Hauptverein"

### Requirement: EventCounter capacity logic tests
The system SHALL test `EventCounter::is_booked_up()` with edge cases.

#### Scenario: max_subscribers -1 means unlimited
- **WHEN** `max_subscribers` is `-1` and any number of subscribers/waiting list exist
- **THEN** `is_booked_up()` returns `false`

#### Scenario: both slots filled
- **WHEN** `subscribers >= max_subscribers` AND `waiting_list >= max_waiting_list`
- **THEN** `is_booked_up()` returns `true`

#### Scenario: only subscriber slots filled
- **WHEN** `subscribers >= max_subscribers` BUT `waiting_list < max_waiting_list`
- **THEN** `is_booked_up()` returns `false`

### Requirement: LifecycleStatus bookability tests
The system SHALL test `LifecycleStatus::is_bookable()` for all variants.

#### Scenario: bookable statuses
- **WHEN** status is `Review`, `Published`, or `Running`
- **THEN** `is_bookable()` returns `true`

#### Scenario: non-bookable statuses
- **WHEN** status is `Draft`, `Finished`, `Closed`, or `Archived`
- **THEN** `is_bookable()` returns `false`

### Requirement: EventType and LifecycleStatus FromStr tests
The system SHALL test string parsing for both enums including error cases.

#### Scenario: valid EventType parsing
- **WHEN** parsing "Fitness" or "Events"
- **THEN** the correct variant is returned

#### Scenario: invalid EventType parsing
- **WHEN** parsing an invalid string like "Invalid"
- **THEN** an error is returned

#### Scenario: valid LifecycleStatus parsing (case-insensitive)
- **WHEN** parsing any valid status string (case-insensitive)
- **THEN** the correct variant is returned

#### Scenario: invalid LifecycleStatus parsing
- **WHEN** parsing an invalid string
- **THEN** an error is returned

### Requirement: NewsTopic tests
The system SHALL test `NewsTopic` display names and conversions.

#### Scenario: display_name returns German name
- **WHEN** `display_name()` is called on each variant
- **THEN** it returns "Allgemein", "Events", or "Fitness" respectively

#### Scenario: FromStr parsing
- **WHEN** parsing "General", "Events", or "Fitness"
- **THEN** the correct variant is returned

### Requirement: ToEuro / FromEuro tests
The system SHALL test currency formatting and parsing with edge cases.

#### Scenario: formatting various amounts
- **WHEN** `to_euro()` is called on BigDecimal values
- **THEN** amounts are formatted as "X,XX €" with German decimal separator

#### Scenario: parsing German-formatted amounts
- **WHEN** `parse_euro_without_symbol()` is called on strings like "1.234,56"
- **THEN** the correct BigDecimal is returned

#### Scenario: rounding behavior
- **WHEN** formatting amounts with more than 2 decimal places
- **THEN** values are rounded to 2 decimal places

### Requirement: EventId serialization roundtrip tests
The system SHALL test EventId encode/decode roundtrip.

#### Scenario: serialize and deserialize roundtrip
- **WHEN** an EventId is serialized to JSON and deserialized back
- **THEN** the original value is preserved

### Requirement: EventType subject_prefix tests
The system SHALL test subject prefix generation.

#### Scenario: Fitness prefix
- **WHEN** `subject_prefix()` is called on `EventType::Fitness`
- **THEN** it returns "[Fitness@SVE]"

#### Scenario: Events prefix
- **WHEN** `subject_prefix()` is called on `EventType::Events`
- **THEN** it returns "[Events@SVE]"

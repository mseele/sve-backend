# Changelog

All notable changes to this project will be documented in this file.

## [2.4.1]

### Bug Fixes

- Update CSV header validation for VobaClassicCSVReader

## [2.4.0] - 2025-07-20

### Features

- Add cors for local development
- Add support for custom fields

## [2.3.1] - 2024-12-19

### Refactor

- Switch from hyper (legacy) to reqwest

## [2.3.0] - 2024-10-24

### Features

- Implement membership application backend

### Refactor

- Improve function name

## [2.2.5] - 2024-10-02

### Bug Fixes

- Use new reploy hook

### Refactor

- Switch to new prebooking url

## [2.2.4] - 2024-05-06

### Bug Fixes

- Remove unused code
- Simplify if statement

### Refactor

- Remove deprecated code

## [2.2.3] - 2024-05-06

### Bug Fixes

- Add space before WHERE block

## [2.2.2] - 2024-05-06

### Bug Fixes

- Correct migrations

## [2.2.1] - 2024-03-06

### Features

- Split subscribers into chunks

## [2.2.0] - 2023-12-23

### Refactor

- Switch to or_default fn

## [2.1.1] - 2023-10-31

### Refactor

- Send messages in bulk

## [2.1.0] - 2023-09-27

### Bug Fixes

- Add linker env
- Remove cors configuration
- Encode filename in content disposition header
- Remove ' ' replacement with '_'
- Remove unnecessary vec! macro
- Switch to new backend url
- Enable https for hyper
- Switch to json log format

### Features

- Integrate lambda_http

### Refactor

- Remove macro
- Bump printpdf to 0.6.0

## [2.0.15] - 2023-07-17

### Bug Fixes

- Skip historic dates in render_schedule_change

### Refactor

- Restrict access to credentials

## [2.0.14] - 2023-07-09

### Bug Fixes

- Correct news subscription subject
- Avoid fetching empty event list

## [2.0.13] - 2023-06-08

### Refactor

- Migrate from actix-web to axum

## [2.0.12] - 2023-04-24

### Bug Fixes

- Add canceled and enrolled to finished event id's statement

## [2.0.11] - 2023-04-24

### Features

- Add task to automatically close finished events

## [2.0.10] - 2023-04-23

### Features

- Add archived event feature

## [2.0.9] - 2023-03-28

### Bug Fixes

- Improve bookings without payment sort order

### Refactor

- Remove deprecated code

## [2.0.8] - 2023-01-06

### Bug Fixes

- Send event reminder only to enrolled subscribers

## [2.0.7] - 2022-12-29

### Bug Fixes

- Trim and filter empty comment

### Features

- Send participation confirmation after finished event

## [2.0.6] - 2022-12-27

### Bug Fixes

- Allow canceled bookings to re-asign
- Use alternative email account for all actions

### Features

- Add participant list export

### Refactor

- Remove unused into_iter() call

## [2.0.5] - 2022-12-22

### Refactor

- Optimize imports
- Improve db connection pool settings

## [2.0.4] - 2022-12-16

### Bug Fixes

- Use correct event for prebooking email

### Refactor

- Inline code

## [2.0.3] - 2022-12-01

### Bug Fixes

- Correct typo
- Correct direct booking variable
- Remove not needed Debug derive

### Features

- Add cost per date field

### Refactor

- Display removed dates in the schedule changed email
- Rename cost to price

## [2.0.2] - 2022-11-22

### Refactor

- Replace excel crate

## [2.0.1] - 2022-11-18

### Bug Fixes

- Add 'Published' lifecycle when fetching reminder events

## [2.0.0] - 2022-11-17

### Bug Fixes

- Correct events view
- Get updated event inside the transaction
- Add event subscribers unique key
- Round cost values to 2 digits after the decimal point
- Disallow booking in some lifecycle states
- Correct typo
- New event has no event dates to delete
- Remove subscriber id from booking struct
- Correct text
- Use false if member is not set
- Sort enrolled before not enrolled
- Correct visibility and function order
- Use booking date for payday calculation
- Correct column names
- Add libclang-dev package
- Add libxlsxwriter-sys dependencies
- Use openssl md5 function to avoid build errors
- Disable md5 feature to avoid build errors
- Correct libxlsxwriter-sys dependencies

### Features

- Work on set/get of events into psql
- Add event counters db view
- Implement get event counters from psql
- Implement booking into psql
- Add sort & lifecycle status option
- Create new EventId struct
- Create new EventId struct
- Migrate the remaining event logic to psql
- Get events by lifecycle status
- Add close date
- Make it possible to send emails per event
- Send emails if event schedule changes
- Optionally add event bookings to event
- Provide update booking payment possibility
- Implement cancel booking
- Send reminder emails if needed
- Integrate unpaid bookings into verify payments result
- Add ability to send payment reminders
- Add excel export of bookings

### Refactor

- Rename news subscription model
- Use connection instead of executor
- Improve iteration of events
- Use event lifecycle_status on booking
- Remove legacy code
- Rename lifecycle status to status
- Remove camel case option
- Rename booking_number to payment_id
- Use Self instead of specific type
- Switch from pub to pub(crate)
- Finish prebooking migration
- Add event lifecycle status "Running"
- Add event lifecycle status "Running"
- Improve function names
- Add event lifecycle status "Running"
- Share subject prefix code
- Split method
- Improve function name
- Inline subject prefix
- Format code
- Check is empty outside of insert_event_dates
- Imrove delete api
- Optimize imports
- Improve naming
- Improve api
- Use handlebars for templates
- Add fetch_events and insert subsribers if requested
- Split verify payment and return of unpaid bookings
- Extract payday calculation from template module
- Simplify code
- Use new struct for unpaid bookings
- Extend unpaid booking attributes
- Use fetch_events
- Add payment account event attribute
- Improve api
- Move subject prefix generation into EventType struct
- Provide the possibility to get subscribers via get_event
- Use static waiting list templates
- Make event payment account optional
- Simplify prebooking url
- Cargo clippy --fix

## [1.3.3] - 2022-07-12

### Bug Fixes

- Correct typo

### Refactor

- Only use as much workers as cpu's

## [1.3.2] - 2022-05-28

### Features

- Add db migrations for events
- Add db migrations for news

### Refactor

- Migrate news subscriptions from firestore to psql
- Migrate news subscriptions from firestore to psql

## [1.3.1] - 2022-05-25

### Bug Fixes

- Avoid missing email adresses

## [1.3.0] - 2022-05-04

### Features

- Implement support for a second csv format

### Refactor

- Move csv parser into separate module
- Remove id from payment record

## [1.2.1] - 2022-04-24

### Bug Fixes

- Add utf-8 encoding for email content

## [1.2.0] - 2022-04-22

### Bug Fixes

- Improve error logging
- Log warn message if booking link is invalid
- Fix wrong if clause when checking prebooking
- Invalidate prebooking for non-beta events
- Add id and sortIndex to appointment
- Fix bcc recipients bug
- Fix possible NPE when using customDate
- Do not use wildcard to work on older browsers
- Remove caching
- Remove race-condition when multiple bookings happen in parallel
- Switch to DEBUG filter
- Query parameter name
- Make logging work again
- Avoid NPE when obj and defaultObj are both null
- Remove appending slash
- Define project id
- Remove unused pub
- Log errors the correct way
- Correct RUST_LOG env variable
- Remove email indentation
- Remove camelCase option from enums
- Use from instead of sender
- Correct email indentation
- Correct newsletter url
- Create directory
- Add tag to docker image
- Add install of ca-certificates
- Add ca-certificates into final image
- Add ' prefix only if we have a phone number
- Compare not all values on prebooking check
- Switch from post to get
- Reduce visibility
- Add date to emails
- Use message_id with localhost
- Email bounce from gmx/web.de
- Use ISO 8859 for csv encoding
- Remove thousands separator
- Remove thousands separator
- Remove euro conversion via steel-cent
- Skip phone number prefix (') for prebooking check
- Log backtraces, finally
- Strip € suffix from "Betrag" values
- Add dot behind short weekday

### Features

- New tools web ui for the backend
- Implement pre-booking support
- Identify duplicate link clicks
- Add refresh post request
- Implement calendar access
- Add notifications hook
- Add watch/stop calendar api calls
- Add html link to appointment
- Trigger re-deploy on calendar change
- Send multiple emails via 1 smtp session
- Add check email connectivity task
- Add renew calendar watch task
- Add new all param for events api
- Make booking button text configurable
- Add support for alternative email addresses as sender
- Implement events store
- Implement first api method
- Add environment variables
- Implement subscription store methods
- Add calendar access
- Implement renew logic
- Add sheets access
- Add a test for the cost function
- Implement has_booking
- Work on email
- Implement events get request
- Implement counter get request
- Return event on write_event
- Implement update post request
- Implement delete post request
- Work on events api and logic
- Work on news api and logic
- Work on events api and logic
- Work on events api and logic
- Work on contact api and logic
- Work on calendar api and logic
- Work on tasks api and logic
- Finish email implementation
- Work on sending emails
- Implement contact email logic
- Implement news email logic
- Add new cost_as_string method
- Implement events email logic
- Add CORS filter
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on github actions
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Work on build
- Generate unique booking number per booking
- Implement verify_payments
- Add support for csv start date

### Refactor

- Move failure message to constant
- Remove unused import
- Reduce files
- Reduce files
- Move credentials into main
- Organize uses
- Sort mods
- Switch from boolean to enum
- Improve event type naming
- Move response error into api module
- Switch from macros to config via code
- Extract logic into separate method
- Switch from explicit type to self
- Optimize imports
- Improve code readability
- Improve readability
- Optimize imports
- Remove unused imports
- Include project id into code
- Switch from include_str! to env!
- Switch back to file based secrets
- Rename headers_indices to header_indices
- Optimize imports
- Destruct json arg directly as method argument
- Reduce clones by avoid borrowing
- Reduce clone by using take
- Introduce ToEuro traid
- Add encoding & mime-type to emails
- Use 4 workers
- Move trait method into trait as default impl
- Switch to json instead of plain text

### Styling

- Format code
- Format code

### Testing

- Add create body test

### Bugfix

- Custom date can be null
- Correct infinite booking

### Build

- Configure conventional commits
- Fix eclipse factorypath
- Move tools into separate repository
- Increase gae version
- Bump google-cloud-logging-logback
- Bump commons-text from 1.8 to 1.9
- Bump google-cloud-firestore from 1.35.1 to 1.35.2
- Increase gae version
- Increase version
- Fix factorypath
- Bump com.diffplug.eclipse.apt from 3.23.0 to 3.24.0
- Bump jettyVersion from 9.4.30.v20200611 to 9.4.31.v20200723
- Bump google-api-services-calendar
- Switch from npm to yarn
- Bump google-cloud-logging-logback
- Bump google-cloud-firestore from 1.35.2 to 2.0.0
- Bump google-api-services-sheets
- Bump com.github.ben-manes.versions from 0.29.0 to 0.33.0
- Bump jerseyVersion from 2.31 to 2.32
- Bump google-api-services-sheets
- Bump com.diffplug.eclipse.apt from 3.24.0 to 3.25.0
- Bump google-auth-library-oauth2-http from 0.21.1 to 0.22.0
- Bump com.github.johnrengelman.shadow from 6.0.0 to 6.1.0
- Bump jettyVersion from 9.4.31.v20200723 to 9.4.32.v20200930
- Bump appengine-gradle-plugin from 2.3.0 to 2.4.1
- Bump google-api-services-calendar
- Bump google-cloud-firestore from 2.0.0 to 2.1.0
- Fix factorypath

<!-- generated by git-cliff -->

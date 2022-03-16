# Changelog

All notable changes to this project will be documented in this file.

<!-- next-header -->
## [Unreleased] - ReleaseDate

### Features

- Implement verify_payments

### Miscellaneous Tasks

- Update dependencies

### Refactor

- Introduce ToEuro traid

### Styling

- Format code

## [1.0.8] - 2022-03-13

### Features

- Implement verify_payments

### Miscellaneous Tasks

- Update dependencies

### Refactor

- Introduce ToEuro traid

### Styling

- Format code

## [1.0.8] - 2022-03-13

### Bug Fixes

- Email bounce from gmx/web.de

### Miscellaneous Tasks

- Release

## [1.0.7] - 2022-03-13

### Bug Fixes

- Use message_id with localhost

### Miscellaneous Tasks

- Release

## [1.0.6] - 2022-03-13

### Bug Fixes

- Add date to emails

### Miscellaneous Tasks

- Release 1.0.6

## [1.0.5] - 2022-03-12

### Bug Fixes

- Reduce visibility

### Miscellaneous Tasks

- Work on release pipeline
- Bump dependencies
- Release

### Refactor

- Optimize imports
- Destruct json arg directly as method argument
- Reduce clones by avoid borrowing
- Reduce clone by using take

### Styling

- Format code

## [1.0.4] - 2022-03-11

### Bug Fixes

- Switch from post to get

### Miscellaneous Tasks

- Release 1.0.4

## [1.0.3] - 2022-03-09

### Bug Fixes

- Add ' prefix only if we have a phone number
- Compare not all values on prebooking check

### Features

- Generate unique booking number per booking

### Miscellaneous Tasks

- Bump dependencies
- Release 1.0.3

### Refactor

- Rename headers_indices to header_indices

## [1.0.2] - 2022-03-09

### Bug Fixes

- Add ca-certificates into final image

### Miscellaneous Tasks

- Release 1.0.2

## [1.0.1] - 2022-03-09

### Bug Fixes

- Add install of ca-certificates

### Miscellaneous Tasks

- Release 1.0.1

## [1.0.0] - 2022-03-09

### Bug Fixes

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

### Features

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

### Miscellaneous Tasks

- Bump gae version from "14" to "15"
- Fix typo
- Initialize rust
- Add actix as server
- Change - to _
- Setup deploy
- Bump actix-web version
- Add TODO'S
- Add comment
- Bump dependencies
- Bump dependencies
- Remove commented out code
- Finalize build system
- Bump dependencies
- Rename github action
- Release 1.0.0
- Fix tag syntax
- Add comment

### Refactor

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

### Testing

- Add create body test

## [1.13.0] - 2021-12-18

### Features

- Add support for alternative email addresses as sender

### Miscellaneous Tasks

- Bump dependencies
- Change button text attribute name
- 1.13.0

## [1.12.0] - 2021-12-07

### Features

- Make booking button text configurable

### Miscellaneous Tasks

- Bump gae version from "13" to "14"
- 1.12.0

## [1.11.1] - 2021-08-26

### Bug Fixes

- Query parameter name

### Miscellaneous Tasks

- 1.11.1

## [1.11.0] - 2021-08-26

### Bug Fixes

- Remove caching
- Remove race-condition when multiple bookings happen in parallel
- Switch to DEBUG filter

### Features

- Add new all param for events api

### Miscellaneous Tasks

- Bump gae version from "10" to "11"
- Move cron.yaml into correct directory
- Bump dependencies
- Bump gae version from "11" to "12"
- Switch to F1 instance
- Work on logging
- Bump gradle to 7.1
- Bump gae version from "12" to "13"
- 1.11.0

## [1.10.0] - 2021-01-09

### Features

- Add check email connectivity task
- Add renew calendar watch task

### Miscellaneous Tasks

- Bump dependencies
- Bump gradle from 6.3 to 6.8
- 1.10.0

### Build

- Bump google-cloud-firestore from 2.0.0 to 2.1.0
- Fix factorypath

## [1.9.2] - 2020-11-08

### Bug Fixes

- Fix possible NPE when using customDate
- Do not use wildcard to work on older browsers

### Miscellaneous Tasks

- Bump appengine version from 9 to 10
- 1.9.2

### Build

- Bump com.github.ben-manes.versions from 0.29.0 to 0.33.0
- Bump com.diffplug.eclipse.apt from 3.24.0 to 3.25.0
- Bump appengine-gradle-plugin from 2.3.0 to 2.4.1
- Bump google-api-services-calendar
- Bump jerseyVersion from 2.31 to 2.32
- Bump google-api-services-sheets
- Bump jettyVersion from 9.4.31.v20200723 to 9.4.32.v20200930
- Bump com.github.johnrengelman.shadow from 6.0.0 to 6.1.0
- Bump google-auth-library-oauth2-http from 0.21.1 to 0.22.0

## [1.9.1] - 2020-08-21

### Bug Fixes

- Fix bcc recipients bug

### Miscellaneous Tasks

- 1.9.1

### Build

- Bump google-cloud-firestore from 1.35.2 to 2.0.0
- Bump google-api-services-sheets

## [1.9.0] - 2020-08-13

### Features

- Trigger re-deploy on calendar change
- Send multiple emails via 1 smtp session

### Miscellaneous Tasks

- 1.9.0

### Build

- Switch from npm to yarn
- Bump google-cloud-logging-logback

## [1.8.0] - 2020-08-11

### Bug Fixes

- Add id and sortIndex to appointment

### Features

- Add notifications hook
- Add watch/stop calendar api calls
- Add html link to appointment

### Miscellaneous Tasks

- 1.8.0

### Build

- Bump google-api-services-calendar
- Bump jettyVersion from 9.4.30.v20200611 to 9.4.31.v20200723
- Fix factorypath
- Bump com.diffplug.eclipse.apt from 3.23.0 to 3.24.0

## [1.7.0] - 2020-07-27

### Features

- Implement calendar access

### Miscellaneous Tasks

- 1.7.0

### Build

- Increase version

## [1.6.0] - 2020-07-27

### Bug Fixes

- Log warn message if booking link is invalid
- Fix wrong if clause when checking prebooking
- Invalidate prebooking for non-beta events

### Features

- Identify duplicate link clicks
- Add refresh post request

### Miscellaneous Tasks

- Bump gae version from "6" to "7"
- 1.6.0

### Build

- Bump google-cloud-firestore from 1.35.1 to 1.35.2
- Bump google-cloud-logging-logback
- Bump commons-text from 1.8 to 1.9
- Increase gae version

## [1.5.1] - 2020-06-26

### Bug Fixes

- Improve error logging

### Miscellaneous Tasks

- 1.5.1

### Build

- Increase gae version

## [1.5.0] - 2020-06-26

### Features

- New tools web ui for the backend
- Implement pre-booking support

### Miscellaneous Tasks

- Simplify changelog
- Fix changelog typo
- Bump google-auth-library-oauth2-http from 0.20.0 to 0.21.0
- Bump google-cloud-logging-logback from 0.117.0-alpha to 0.118.0-alpha
- Bump appengine-gradle-plugin from 2.2.0 to 2.3.0
- Bump google-cloud-firestore from 1.34.0 to 1.35.0
- Bump com.diffplug.eclipse.apt from 3.22.0 to 3.23.0
- Bump com.github.johnrengelman.shadow from 5.2.0 to 6.0.0
- Bump jettyVersion from 9.4.29.v20200521 to 9.4.30.v20200611
- 1.5.0

### Refactor

- Move failure message to constant

### Build

- Fix eclipse factorypath
- Move tools into separate repository

## [1.4.2] - 2020-06-09

### Miscellaneous Tasks

- 1.4.2

### Bugfix

- Custom date can be null
- Correct infinite booking

### Build

- Configure conventional commits

<!-- next-url -->
[Unreleased]: https://github.com/mseele/sve-backend/compare/v1.0.8...HEAD
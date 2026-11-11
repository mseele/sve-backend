# SVE Web Backend

SVE Website Backend written in Rust and deployed as Lambda on AWS

## Scheduled tasks

The external daily scheduler must invoke `GET /api/tasks/send_manager_event_reminders`
with the existing `x-api-key` task credential. This task sends manager reminders
seven calendar days before eligible Events start. Scheduler configuration is
maintained outside this repository.

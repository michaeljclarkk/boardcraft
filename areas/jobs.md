---
id: jobs
name: Durable jobs
order: 11
percent: 0
code: crates/jobs
---
# Durable jobs

Background scheduling that survives restart: the Hangfire equivalent. Enqueue only after the owning transaction commits, and make retries safe.

## Workflows

- [ ] Enqueue work with a committed change
- [ ] Run it after restart
- [ ] Retry a failure without replaying the effect twice
- [ ] Inspect and cancel a queued job

## Notes

Not started. Create `crates/jobs` when automation or notifications need durable work.

# ADR-0002: Durable compensation for managed project creation

- Status: Accepted
- Date: 2026-08-09

## Context

Harness repositories, access tokens and managed directories are external resources and cannot participate in the PostgreSQL transaction that creates a Relay project.

## Decision

After the Human's Harness account is confirmed and before project repository/token provisioning starts, Relay persists a `project_provisioning_cleanup_jobs` record containing only non-secret resource identifiers. The creating request owns an initial provisioning lease so the worker cannot race a long-running Git push. Successful project and Git configuration creation marks that record completed in the same database transaction. Known failures release the lease immediately; interrupted or abandoned provisioning becomes eligible when its lease expires.

Cleanup is idempotent, treats missing resources as success, retries with capped exponential backoff and never persists access-token plaintext.

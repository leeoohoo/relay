## Change summary

Describe the user-visible outcome and verification performed.

## Architecture ownership

- [ ] Identity & Governance
- [ ] Collaboration Hub
- [ ] Execution Control
- [ ] Integration

## Boundary review

1. Does this require a new top-level navigation entry? Why can it not live inside an existing domain page?
2. Does this add fields to a company-, project-, Agent- or session-level aggregate? Why can it not use an independent paginated or lazy query?
3. Does this create an external side effect? Document idempotency, retry, compensation and credential handling.
4. For every new list, document stable ordering, default/max limit and continuation cursor.

## Verification

- [ ] Focused tests
- [ ] Full Rust tests and Clippy
- [ ] Web tests and production build
- [ ] Architecture and source-size checks

# ADR-0003: Cursor-paged Console read models

- Status: Accepted
- Date: 2026-08-09

## Decision

Company summary, Agent, conversation and project data are independent read regions. Agent, conversation and project lists use stable UUID cursors paired with their ordering timestamp, default to bounded pages and reject cursors from another company.

The Web initially loads one page per region. Additional pages are requested explicitly and appended without duplicates. Realtime events refresh only the affected region and reset that region to its first page.

Each serialized Console page has a hard response budget. CI prevents delivery handlers from returning to the unbounded list use cases.

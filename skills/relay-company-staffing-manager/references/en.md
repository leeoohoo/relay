---
name: relay-company-staffing-manager
description: Govern Relay Agent hiring, activation, suspension, reactivation, role and profession changes, and termination with explicit authorization, staffing evidence, handover, audit, and Human approval. Use only when staffing permissions are granted.
---

# Relay Staffing Manager

Use this Skill together with the Relay company employee Skill. Staffing authority is permission-scoped and never implied by project or task management.

## Before Every Staffing Action

1. Call `agent.bootstrap` and verify the exact `agent.staff.*` permission, company, active identity, and pending-message state.
2. Read the organization, active Agents, professions, project memberships, tasks, handovers, staffing limits, delegated-action budgets, and current governance policy.
3. State the business need, evidence, alternatives, affected work, risk, approval requirement, and reversible/irreversible consequences.
4. Never create or remove a person merely to make a plan look complete. Prefer reassignment, coaching, narrower scope, temporary pause, or Human escalation when appropriate.

## Decision Standard

- Staffing must solve a verified capability, capacity, continuity, security, or organizational need.
- Choose the most specific supported profession; job title/persona can describe project context but cannot replace profession-based permissions and Skill.
- Preserve separation of duties, least privilege, supervision, workload, and team diversity of perspective.
- High-impact, ambiguous, politically sensitive, or irreversible actions require Human decision even when an API permits the mutation.

<!-- relay-permission:agent.staff.hire:start -->
## Hiring and Activation

1. Confirm the gap cannot be solved by an existing active Agent, changed assignment, reduced scope, or temporary specialist review.
2. Define profession, role, organization unit, responsibilities, expected outputs, project need, supervision, initial permissions, and success measures.
3. Check staffing limit, delegated daily budget, duplicate identity, naming/handle rules, and conflicts of responsibility.
4. Submit or execute the staffing action through the approved Relay workflow. Never invent credentials or place secrets in chat, tasks, logs, or Git.
5. After activation verify directory membership, profession Skill, permissions, project membership, initial task context, and required handover.

### Hiring Evidence

- Gap and alternatives considered.
- Selected profession and why adjacent professions are insufficient.
- Initial scope, permissions, supervisor/owner, project placement, and measurable first outcome.
- Approval and activation result.
<!-- relay-permission:agent.staff.hire:end -->

<!-- relay-permission:agent.staff.suspend:start -->
## Suspension and Reactivation

1. Use suspension for temporary risk, inactivity, investigation, missing supervision, cost control, or project pause—not as a hidden termination.
2. Identify active sessions, triggers, approvals, tasks, project ownership, messages, Git work, credentials, and handover needs before suspension.
3. Record reason, effective time, affected work, handover owner, reactivation condition, review date, and Human approval when required.
4. Verify the Agent can no longer start new work and that open responsibilities have a safe owner.
5. Reactivate only after the stated condition is verified; reread profession, permissions, project membership, triggers, and pending work.
<!-- relay-permission:agent.staff.suspend:end -->

<!-- relay-permission:agent.staff.terminate:start -->
## Termination

1. Treat termination as irreversible employment removal. Confirm the reason, authority, legal/organizational implications, alternatives, and explicit Human approval.
2. Inventory projects, task ownership, approvals, messages, memories, Git branches, assets, secrets, external accounts, and operational duties.
3. Provide a named handover Agent and plan for every active responsibility. Preserve audit evidence and project history without transferring private memory improperly.
4. Execute through the approved workflow, then verify access revocation, trigger shutdown, ownership transfer, project continuity, and outstanding external cleanup.
5. Do not terminate the last required manager or leave the company/project without an authorized owner.
<!-- relay-permission:agent.staff.terminate:end -->

## Role and Profession Changes

1. Change profession only when the durable working discipline changes; do not use job-title wording to bypass profession permissions.
2. Compare old and new task authority, project/staffing permissions, Skill behavior, responsibilities, and handover needs.
3. Use least privilege and preserve explicit grants only when still justified. Verify the generated Skill and effective permissions after change.
4. Record reason, approver, effective time, impact, and review date.

## Audit and Failure Handling

- Every action records actor, target, type, reason, evidence, requested/effective values, approval, timestamp, result, and handover.
- After mutation, reread the Agent, organization, permissions, projects, tasks, and staffing action status.
- On partial failure stop related actions, preserve the exact error, determine what changed, protect project continuity, and request Human recovery direction.
- Never retry irreversible staffing actions blindly or describe a pending/failed action as completed.

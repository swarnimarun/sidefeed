# Sidefeed V3: rewrite plan and first implementation

V3 is a ground-up, self-hostable feed-management, reading and publishing system for inexpensive VPSs and home servers.

## Required product scope

- Complete RSS/Atom/JSON Feed subscription, organization, reading, search, saved items and import/export.
- AI classification, embeddings, semantic discovery and suggestions, with bounded jobs and explicit provider/privacy controls.
- Networked readers that share cached updates: publisher push where available, coordinated polling otherwise, durable push and reconnect catch-up.
- Publishing and microblogging with drafts, explicit publish actions, visibility controls and durable delivery.
- Bidirectional ActivityPub with interoperability, authentication, moderation and recovery tests.
- ATProto readers, thread discovery and authenticated publishing/sync; auditable approval-gated agent discovery and trusted fleets.
- Simple single-node hosting, backups, upgrades, rollback and measured low-resource profiles.

These capabilities are required V3 scope. Milestones sequence the work; they do not remove features from the product.

## Runnable first slice

The new implementation lives in [`v3/`](../../v3/), using Node 24 built-in HTTP/SQLite and a dependency-light web UI. Start with the [run guide](IMPLEMENTATION.md), [verification report](QA.md) and [ATProto/agent/fleet contracts](INTEGRATIONS.md). Forty-nine automated tests pass locally. Opt-in model jobs and partial ATProto thread snapshots are implemented with fixture coverage. [stage-2 CI run](https://github.com/swarnimarun/sidefeed/actions/runs/41207828117) passed at `3fbbbde58b5c97ccd83f62d0fce68973a5cb8f85`: 49 tests, 15 syntax checks, scripted desktop/mobile browser flows and container build/start/token smoke. Desktop reader/thread, mobile reader and fully loaded mobile sources screenshots were reviewed. Live model/provider quality, full social federation and production readiness are not claimed.

## Read in this order

1. [Architecture and implementation plan](ARCHITECTURE-PLAN.md): proposed architecture, domain contracts, security, milestones and acceptance gates.
2. [Operations contract](OPERATIONS-DRAFT.md): deployment profiles, backups, safe restore, limits and failure drills.
3. [Contribution and release procedure](CONTRIBUTING-RELEASE-DRAFT.md): CI, review, release and deployment gates.
4. [Operations issue pack](ISSUES-OPS-DRAFT.md): ten actionable work packages with acceptance checklists, not yet filed as separate issues.

## Branch and verification status

The `v3` branch starts at `115d6d03fbb4f4929eadd9b9f3e2a65e472f1bf5`. Existing `main` and the older `v2` prototype are preserved. The historical root implementation remains separate from the new executable `v3/` tree.

The first executable slice is now in `v3/`; the historical root runtime remains a reference. This is not the complete V3 release. The CI image passed smoke tests for the exact commit above; no production rollout or release-grade image qualification is claimed. Implementation replaces the historical runtime deliberately, preserving history, attribution and an explicit data-export/import path. No destructive pruning, old PR closure or external social publication has been performed.

Deployment host, domain/exposure, provider credentials and any spending remain to be selected. Resource sizes in these documents are test targets, not proven minimums. Actual deployment and release require verification of the exact source commit and image digest.

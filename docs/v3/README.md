# Sidefeed V3 rewrite plan

V3 is a ground-up, self-hostable feed-management, reading and publishing system for inexpensive VPSs and home servers.

## Required product scope

- Complete RSS/Atom/JSON Feed subscription, organization, reading, search, saved items and import/export.
- AI classification, embeddings, semantic discovery and suggestions, with bounded jobs and explicit provider/privacy controls.
- Networked readers that share cached updates: publisher push where available, coordinated polling otherwise, durable push and reconnect catch-up.
- Publishing and microblogging with drafts, explicit publish actions, visibility controls and durable delivery.
- Bidirectional ActivityPub with interoperability, authentication, moderation and recovery tests.
- Simple single-node hosting, backups, upgrades, rollback and measured low-resource profiles.

These capabilities are required V3 scope. Milestones sequence the work; they do not remove features from the product.

## Read in this order

1. [Architecture and implementation plan](ARCHITECTURE-PLAN.md): proposed architecture, domain contracts, security, milestones and acceptance gates.
2. [Operations contract](OPERATIONS-DRAFT.md): deployment profiles, backups, safe restore, limits and failure drills.
3. [Contribution and release procedure](CONTRIBUTING-RELEASE-DRAFT.md): CI, review, release and deployment gates.
4. [Operations issue pack](ISSUES-OPS-DRAFT.md): ten actionable work packages with acceptance checklists, not yet filed as separate issues.

## Branch and verification status

The `v3` branch starts at `115d6d03fbb4f4929eadd9b9f3e2a65e472f1bf5`. Existing `main` and the older `v2` prototype are preserved. The source tree currently contains the historical implementation; adding this plan does not turn it into the V3 rewrite.

This is the planning baseline. No V3 application code, executable CI, deployment image or production rollout is claimed complete. Implementation will replace the historical runtime deliberately, preserving history, attribution and an explicit data-export/import path. No destructive pruning, old PR closure or live publication is implied by this planning commit.

Deployment host, domain/exposure, provider credentials and any spending remain to be selected. Resource sizes in these documents are test targets, not proven minimums. Actual deployment and release require verification of the exact source commit and image digest.

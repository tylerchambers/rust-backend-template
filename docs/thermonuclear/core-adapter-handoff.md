# CORE-1 runtime adapter coordination (action required)

Core owns application/projects.rs, memberships.rs, tests and infrastructure/lib.rs. Existing transaction implementations actually live in infrastructure/projects.rs (runtime-owned). No edits there from core.

Planned narrow application contracts:
- `TenantAuthority::current_role` extracted from `OrganizationRepository`.
- `TenantTransaction: Send` owns only audit and commit.
- `ProjectRepository` additionally owns `project_changed`.
- `ProjectTransaction: TenantAuthority + ProjectRepository + TenantTransaction` with blanket implementation.
- `OrganizationTransaction: TenantAuthority + OrganizationRepository + TenantTransaction` with blanket implementation.
- `MembershipTransaction: TenantAuthority + MembershipRepository + TenantTransaction` with blanket implementation in memberships module.
- `ProjectUnitOfWork::begin_projects`, `OrganizationUnitOfWork::begin_organization`, `MembershipUnitOfWork::begin_memberships`, each returning its narrow transaction trait object.

Core will implement all three Database factories in infrastructure/lib.rs with one private constructor returning the existing concrete TenantWork (one SQL transaction).

Runtime-owned infrastructure/projects.rs changes needed:
1. Remove old `impl UnitOfWork for Database` and old UnitOfWork import (factory moves to lib.rs).
2. Import `TenantAuthority`.
3. Move `current_role` method from `impl OrganizationRepository for TenantWork` to `impl TenantAuthority for TenantWork` (retain SQL and runtime locking fixes).
4. Move `project_changed` method from `impl TenantTransaction for TenantWork` into `impl ProjectRepository for TenantWork` unchanged.
5. Keep audit and commit in `impl TenantTransaction` unchanged.

These are trait-boundary moves, not SQL changes. Preserve runtime implementer's authority locking fixes. Parent/runtime must apply these coordinated adapter moves before infrastructure compile/integration gates. Core will not edit runtime-owned file without permission.

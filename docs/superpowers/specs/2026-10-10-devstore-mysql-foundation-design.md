# DevStore MySQL Foundation and Public-Launch Readiness Design

Date: 2026-10-10

Status: Draft for user review; supersedes the PostgreSQL choice in the initial tutorial-checkout design for future implementation.

## Goal

Move DevStore's Rust persistence layer from PostgreSQL to MySQL and prepare safe local and public deployment configurations for `bz-store.local` and `store.bengzi.cn`.

## User decisions and constraints

- Use MySQL for DevStore, with a dedicated database named `bz_ztore` on the supplied RDS instance.
- Do not use the existing ERP database or its credentials in DevStore.
- Local development connects to the RDS instance; do not add a localhost MySQL service to the normal development stack.
- Keep database and service credentials outside Git in local environment files or a production secret manager. The previously pasted database password is exposed and must be rotated; it must not be reused or copied into project files.
- Use `http://bz-store.local` as the local site origin (with a local hosts-file entry) and `https://store.bengzi.cn` as the public origin.
- Email-provider credentials are not available yet. The application must support a generic SMTP configuration and a local-only mail sink for development and tests.

## Scope

### MySQL conversion

- Replace SQLx PostgreSQL features, `PgPool`, PostgreSQL migrations, and PostgreSQL-specific queries with SQLx MySQL equivalents.
- Convert the entire existing schema and every runtime query, fixture, integration test, test helper, seed path, and CLI operation. The migration is complete only when all existing transaction, ownership, idempotency, entitlement, upload, and audit behavior passes against MySQL.
- Store Rust UUID identifiers in `BINARY(16)` using SQLx's MySQL UUID encoding. Use MySQL `JSON`, `DATETIME(6)` in UTC, and integer cents for money.
- Replace PostgreSQL-only constructs explicitly: positional `$n` parameters, `RETURNING`, `ON CONFLICT`, partial unique indexes, aggregate `FILTER`, `pgcrypto`, `search_path`, `to_jsonb`/`jsonb_build_object`, PL/pgSQL update triggers, regular-expression operators, and advisory transaction locks. Use SQLx `?` parameters, application-generated UUIDs and explicit IDs, `ON DUPLICATE KEY UPDATE`, generated nullable keys for conditional uniqueness, conditional aggregates, explicit JSON projection, application-managed `updated_at`, MySQL `REGEXP`, and row/unique-key locking or naturally idempotent seed operations.
- Avoid relying on server-generated UUIDs or dynamic timestamp defaults. Generate UUIDs in Rust and bind them as SQLx MySQL `BINARY(16)` values; calculate order and token expirations in the application and bind UTC timestamps.
- Preserve `SELECT ... FOR UPDATE` transaction boundaries for payment review, order cancellation/expiry, refresh-token rotation, and other concurrency-sensitive flows. Verify engine and table configuration use InnoDB.
- Treat existing application data as disposable local/test data unless a separate migration is explicitly designed. Never import, alter, or migrate ERP data.

### Development and test database isolation

- Normal local runtime uses a `DATABASE_URL` for the dedicated RDS database `bz_ztore`; no credential appears in committed files or command-line examples.
- Destructive integration tests require a separately provisioned database, expected to be named `bz_ztore_test`, and fail closed unless the configured database name is explicitly test-only. The test harness must not drop, truncate, or migrate the runtime database. Local integration tests may target this dedicated RDS test database; they must not depend on a localhost MySQL instance.
- CI uses an ephemeral MySQL service and a dedicated test database. Tests run serially and clear only data inside that disposable database where MySQL cannot provide PostgreSQL-style per-test schemas.
- Document the exact privileges needed for application migrations versus test database creation/cleanup. Runtime credentials should not have privileges to create or drop databases.

### Launch readiness

- Add dependency vulnerability checks to CI for the locked Rust and npm dependency graphs. High-severity findings block release unless an exact package, reachability, mitigation, and expiry are documented and reviewed. Do not conceal advisories with broad audit suppression.
- Add operator procedures for encrypted database backups, retention, off-host storage, and restore drills. Document object-storage backup separately because a database dump does not include uploaded objects.
- Configure the gateway for local `bz-store.local` and public `store.bengzi.cn`, including HTTPS redirect and trusted proxy behavior. Certificates and private keys are mounted/provisioned outside the repository. Public certificate issuance and DNS validation require the operator's DNS and server access.
- Add production environment validation so the public site origin is `https://store.bengzi.cn`, secure cookies are enabled, and database/storage endpoints are private and TLS-protected where supported.
- Verify CI workflows on the branch and document any checks that still require the GitHub-hosted environment.

## Database and deployment invariants

- The DevStore database is isolated from ERP and other applications.
- No real DSN, password, SMTP secret, TLS key, database dump, or uploaded customer data is committed or printed in logs.
- Migrations are forward-only and tested against a clean MySQL database. Runtime startup does not reset data and does not run destructive test setup.
- Money remains integer cents; audit records and unique payment references remain transactionally consistent.
- Production is not declared ready until DNS, HTTPS, backups, and a successful restore drill have been verified on the real deployment environment.

## Acceptance criteria

1. SQLx builds with the MySQL driver only for runtime persistence; no PostgreSQL-only query or migration remains in active code.
2. A clean MySQL database migrates successfully and the full Rust integration suite passes against MySQL.
3. Concurrency tests still prove idempotent order creation, mutually exclusive payment review/expiry, and exactly-once entitlement grants.
4. The local runtime config targets the dedicated RDS database without embedding credentials; a destructive test configuration pointing at `bz_ztore` is rejected before connecting or changing data.
5. CI starts its own MySQL service, runs migration and integration tests, dependency checks, web/admin builds, and desktop/mobile browser tests.
6. Gateway host routing and HTTPS configuration supports the local and public hostnames without committing certificates.
7. Backup documentation includes a tested restore procedure and separately identifies object storage recovery.
8. Release notes distinguish locally validated behavior from DNS, certificate, RDS, and backup checks that require the user's infrastructure.

## Out of scope

- Connecting to or changing the supplied RDS instance during implementation without an explicit safe test target and rotated credentials.
- Reusing or migrating the ERP database.
- Provisioning DNS records, cloud infrastructure, TLS certificates, or secret-manager entries without the corresponding account access.
- Email verification, password reset, and administrator MFA; these are specified separately.
- Digital downloads, service tickets, refunds, and automated payment gateways.

## Risks and decisions to verify during implementation planning

- RDS engine version, SQL mode, TLS settings, and database privileges must be confirmed before selecting exact migration syntax.
- MySQL has different locking, index, JSON, and uniqueness semantics from PostgreSQL; passing compilation alone is insufficient.
- A dedicated test database and restricted test credentials must exist before any test can create or clean schema objects.
- Local host resolution and trusted certificates require developer-machine setup; public DNS and certificates require operator access.

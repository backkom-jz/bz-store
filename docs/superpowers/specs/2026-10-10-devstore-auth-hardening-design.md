# DevStore Email Authentication and Administrator MFA Design

Date: 2026-10-10

Status: Draft for user review; depends on the MySQL foundation design.

## Goal

Add verifiable email ownership, secure password recovery, and enforced second-factor authentication for administrator access without requiring a mail-provider account during implementation.

## User decisions and constraints

- SMTP credentials are not available yet; support a provider-neutral SMTP configuration and leave credentials as deployment-time secrets.
- Local site origin is `bz-store.local`; public site origin is `store.bengzi.cn`.
- The persistence layer will use the dedicated MySQL database `bz_ztore` described in the foundation design.
- Existing cookie sessions, CSRF checks, Argon2id password hashes, rate limits, and server-side role enforcement remain the foundation.

## Email delivery

- Define a small email-delivery boundary with SMTP as the production adapter and a local mail sink for development/test environments.
- Add an outbox so account creation and token state commit atomically before mail delivery. Delivery failures remain retryable and are observable without logging message secrets or full token URLs.
- Production startup validates SMTP configuration before enabling registration and password-reset email flows. It must fail clearly or keep those flows unavailable; it must never silently report an email as sent.
- Never include SMTP credentials or real reset/verification tokens in logs. Local test messages are available only through the local mail sink and never exposed in production responses.

## Email verification and account registration

- Registration normalizes the email, hashes the password, creates an unverified account, and creates a high-entropy one-time verification token. Store only a cryptographic digest of the token and its purpose/expiry/consumption metadata.
- The registration response does not disclose whether an address already has an account and does not create an authenticated session until verification succeeds.
- Verification is one-time, expiry-checked, rate-limited, and atomically marks the account verified while consuming the token.
- Login for an unverified account returns the same public authentication error shape used for invalid credentials.
- Existing accounts created before this migration need an explicit compatibility decision in the implementation plan; no account should accidentally become inaccessible without a documented backfill or verification policy.

## Password recovery

- `POST /api/v1/auth/password/forgot` always returns the same status and response body for known and unknown email addresses.
- For known verified accounts, generate a high-entropy one-time reset token, store only its digest and a short expiry, and enqueue an email.
- `POST /api/v1/auth/password/reset` atomically validates and consumes the token, updates the Argon2id password hash, and revokes every active session for that user.
- Expired, consumed, malformed, or unknown tokens produce the same public failure response. Rate limits apply by client IP and normalized email digest.
- Reset links use the configured canonical site origin, never an untrusted Host or return URL.

## Administrator TOTP MFA

- TOTP is mandatory for every administrator API request. Frontend route guards are usability only; the Rust API enforces MFA verification.
- Administrator login requires password and a current TOTP code. An administrator without enrolled MFA cannot obtain a normal administrator session.
- Enrollment is performed through a controlled bootstrap/CLI process for an initial admin and a verified re-enrollment process for an existing admin. The process displays the seed/recovery codes once and requires a valid TOTP confirmation before activation.
- Store the TOTP seed encrypted with a dedicated `MFA_ENCRYPTION_KEY`; keep that key outside the database and repository. Store recovery-code digests, never plaintext codes.
- A session records the time and method of successful MFA verification. Admin middleware requires a valid, unexpired MFA marker; ordinary user sessions remain unchanged.
- Recovery codes are single-use and atomically consumed. Re-enrollment invalidates old recovery codes and prior admin sessions and is written to the admin audit log.
- If the encryption key is missing or invalid in production, administrator login and MFA management fail closed.

## API and frontend behavior

- Add email verification resend and password forgot/reset pages with generic success messaging and no account-existence oracle.
- Add TOTP enrollment, confirmation, challenge, and recovery-code endpoints with CSRF protection and explicit rate limits.
- Admin UI displays setup/challenge state and does not treat a hidden route as an authorization boundary.
- Document account recovery and MFA recovery operational procedures, including what support can and cannot do.

## Acceptance criteria

1. Unverified users cannot log in or create orders; valid one-time email verification activates the account.
2. Duplicate/unknown registration and password-forgot requests do not expose account existence in public response bodies/statuses.
3. Verification/reset tokens are stored only as digests, expire, cannot be replayed, and are consumed atomically under concurrent requests.
4. Password reset changes the Argon2id hash and revokes all sessions; old access and refresh cookies stop working.
5. Admin credentials without TOTP cannot access any admin endpoint, including direct API requests; valid TOTP can establish an MFA-marked session.
6. Expired/replayed TOTP windows and consumed recovery codes are rejected; concurrent recovery-code use succeeds at most once.
7. TOTP seed is encrypted in MySQL, recovery-code plaintext is never persisted, and neither secrets nor tokens appear in logs or production responses.
8. SMTP outage leaves a retryable outbox record and does not falsely report delivery; local mail-sink tests can complete verification and recovery without public credentials.
9. Existing cookie, CSRF, ownership, payment-review, and purchase E2E tests continue to pass after the authentication changes.

## Out of scope

- Selecting or provisioning a specific email vendor account.
- SMS authentication, passkeys/WebAuthn, social login, or consumer MFA.
- Support agents bypassing MFA or manually marking arbitrary addresses verified.
- Changing order/payment behavior except that unverified accounts cannot initiate new authenticated purchases.

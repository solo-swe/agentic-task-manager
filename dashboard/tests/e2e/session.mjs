// The session token the e2e server puts in its page, as atm serve puts its own (ATM-572; atm-api's with_session): the
// review changes the page sends carry it, which the specs check (tests/e2e/reviews.spec.ts)
export const E2E_SESSION = 'e2e0'.repeat(16);

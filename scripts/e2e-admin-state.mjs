import { chmodSync, readFileSync, writeFileSync } from 'node:fs';

// Playwright uses one worker at a time. Persist before returning the code so a
// subsequent project or restarted worker cannot reuse the same time step.
export function claimAdminCounter(file, counter) {
  const state = JSON.parse(readFileSync(file, 'utf8'));
  if (!Number.isSafeInteger(counter) || counter < 0 || !Number.isSafeInteger(state.mfaCounter) || state.mfaCounter < 0) {
    throw new Error('invalid private E2E authentication state');
  }
  if (counter <= state.mfaCounter) return false;
  chmodSync(file, 0o600);
  writeFileSync(file, JSON.stringify({ ...state, mfaCounter: counter }), { mode: 0o600 });
  return true;
}

import { describe, expect, it } from 'vitest';
import { canSubmitApproval } from './review';

describe('payment review form', () => {
  const ready = { amount: '99.00', channel: 'bank', reference: 'BANK-1001', confirmed: true };

  it('stays closed until the receipt is explicitly confirmed', () => {
    expect(canSubmitApproval({ ...ready, confirmed: false })).toBe(false);
    expect(canSubmitApproval({ ...ready, amount: '' })).toBe(false);
    expect(canSubmitApproval({ ...ready, channel: '' })).toBe(false);
    expect(canSubmitApproval({ ...ready, reference: '  ' })).toBe(false);
  });

  it('accepts an exact yuan amount only after every field is present', () => {
    expect(canSubmitApproval(ready)).toBe(true);
    expect(canSubmitApproval({ ...ready, amount: '99.001' })).toBe(false);
    expect(canSubmitApproval({ ...ready, amount: '99' })).toBe(true);
  });
});

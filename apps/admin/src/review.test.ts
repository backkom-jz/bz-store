import { describe, expect, it } from 'vitest';
import { approvalCents, canSubmitApproval } from './review';
import { money } from '@devstore/ui/format';

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


describe('one-cent manual WeChat payment', () => {
  it('displays and submits exactly one integer cent', () => {
    expect(approvalCents('0.01')).toBe(1);
    expect(money(1)).toContain('0.01');
    expect(canSubmitApproval({ amount: '0.01', channel: 'wechat', reference: 'WECHAT-MANUAL-FLOW-001', confirmed: true })).toBe(true);
  });

  it('requires a receipt check and a transaction reference even for one cent', () => {
    const payment = { amount: '0.01', channel: 'wechat', reference: 'WECHAT-MANUAL-FLOW-001', confirmed: true };
    expect(canSubmitApproval({ ...payment, confirmed: false })).toBe(false);
    expect(canSubmitApproval({ ...payment, reference: '' })).toBe(false);
    expect(canSubmitApproval({ ...payment, amount: '0.001' })).toBe(false);
  });
});

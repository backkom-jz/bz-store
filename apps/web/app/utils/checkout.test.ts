import {describe, expect, it} from 'vitest';
import {checkoutSteps, formatRemaining, instructionsReady, receiptError, remainingMs, shouldPoll, yuan} from './checkout';

describe('checkout cashier', () => {
  it('advances the visible steps with the order', () => {
    expect(checkoutSteps('pending_payment')).toEqual(['done', 'current', 'upcoming', 'upcoming']);
    expect(checkoutSteps('payment_review')).toEqual(['done', 'done', 'current', 'upcoming']);
    expect(checkoutSteps('completed')).toEqual(['done', 'done', 'done', 'done']);
    expect(checkoutSteps('cancelled')).toEqual(['done', 'upcoming', 'upcoming', 'upcoming']);
  });

  it('polls only while a person is still reviewing or opening access', () => {
    expect(shouldPoll('payment_review')).toBe(true);
    expect(shouldPoll('pending_payment')).toBe(false);
    expect(shouldPoll('completed')).toBe(false);
  });

  it('formats the payment window and the exact transfer amount', () => {
    expect(remainingMs('not-a-date', 0)).toBe(0);
    expect(formatRemaining(0)).toBe('已结束');
    expect(formatRemaining(90 * 60 * 1000)).toBe('01:30:00');
    expect(formatRemaining(49 * 60 * 60 * 1000)).toBe('2 天 1 小时');
    expect(yuan(9900)).toBe('99.00');
    expect(yuan(10)).toBe('0.10');
  });

  it('rejects receipts the server will not accept', () => {
    expect(receiptError({type: 'image/png', size: 12, name: 'a.png'})).toBeNull();
    expect(receiptError({type: '', size: 12, name: 'shot.JPG'})).toBeNull();
    expect(receiptError({type: 'application/pdf', size: 12, name: 'a.pdf'})).toContain('PNG');
    expect(receiptError({type: 'image/jpeg', size: 5 * 1024 * 1024 + 1, name: 'big.jpg'})).toContain('5 MiB');
    expect(receiptError({type: 'image/png', size: 0, name: 'empty.png'})).toContain('空文件');
  });

  it('requires a complete server payment instruction that matches the order amount', () => {
    expect(instructionsReady({channel: 'bank', payee: '测试', account: 'NO-REAL', amount_cents: 9900}, 9900)).toBe(true);
    expect(instructionsReady({channel: 'bank', payee: '测试', account: 'NO-REAL', amount_cents: 100}, 9900)).toBe(false);
    expect(instructionsReady({channel: 'bank', payee: ' ', account: 'NO-REAL'}, 9900)).toBe(false);
  });
});

import { flushPromises, mount } from '@vue/test-utils';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AdminPayment } from '@devstore/shared-types';
import ReviewPaymentDialog from './ReviewPaymentDialog.vue';

const { request } = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock('../session', () => ({ api: { request } }));

const payment: AdminPayment = {
  id: 'proof-one-cent', order_id: 'order-one-cent', order_no: 'DS-ONE-CENT',
  amount_cents: 1, payment_channel_expected: 'wechat', product_title: '微信一分钱流程验证',
  user_email: 'buyer@example.test', status: 'pending', created_at: '2026-10-17T08:00:00Z',
  reviewed_at: null, review_note: null, overdue: false, transaction_reference: null,
  actual_amount_cents: null, payment_channel: null,
};

const modalDescriptor = Object.getOwnPropertyDescriptor(HTMLDialogElement.prototype, 'showModal');
beforeAll(() => {
  Object.defineProperty(HTMLDialogElement.prototype, 'showModal', { configurable: true, value() {} });
});
afterAll(() => {
  if (modalDescriptor) Object.defineProperty(HTMLDialogElement.prototype, 'showModal', modalDescriptor);
  else Reflect.deleteProperty(HTMLDialogElement.prototype, 'showModal');
});
beforeEach(() => {
  request.mockReset();
  request.mockResolvedValue(new Blob(['local receipt fixture'], { type: 'image/png' }));
  vi.stubGlobal('URL', class extends URL {
    static createObjectURL() { return 'blob:local-test-receipt'; }
    static revokeObjectURL() {}
  });
});
afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); });

describe('one-cent WeChat manual review flow', () => {
  it('keeps approval disabled until the operator explicitly verifies the receipt', async () => {
    const wrapper = mount(ReviewPaymentDialog, { props: { payment } });
    await flushPromises();
    expect(wrapper.text()).toContain('0.01');
    expect(wrapper.text()).toContain('微信收款');
    await wrapper.get('#actual-amount').setValue('0.01');
    await wrapper.get('#channel').setValue('wechat');
    await wrapper.get('#reference').setValue('WECHAT-LOCAL-FLOW-001');
    expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined();
    expect(request.mock.calls.every(([, options]) => !options || options.method !== 'POST')).toBe(true);
    await wrapper.get('#confirmed').setValue(true);
    await wrapper.get('form').trigger('submit');
    await flushPromises();
    expect(request).toHaveBeenLastCalledWith('/admin/payments/proof-one-cent/approve', {
      method: 'POST', body: { actual_amount_cents: 1, payment_channel: 'wechat', transaction_reference: 'WECHAT-LOCAL-FLOW-001', receipt_confirmed: true },
    });
    expect(wrapper.emitted('reviewed')).toEqual([['approved']]);
    wrapper.unmount();
  });

  it('records a rejection reason so an invalid receipt can be resubmitted', async () => {
    const wrapper = mount(ReviewPaymentDialog, { props: { payment } });
    await flushPromises();
    await wrapper.get('#reason').setValue('微信账单中未查到对应一分钱入账，请重新提交');
    await wrapper.get('form.reject').trigger('submit');
    await flushPromises();
    expect(request).toHaveBeenLastCalledWith('/admin/payments/proof-one-cent/reject', {
      method: 'POST', body: { reason: '微信账单中未查到对应一分钱入账，请重新提交' },
    });
    expect(wrapper.emitted('reviewed')).toEqual([['rejected']]);
    wrapper.unmount();
  });
});

it('blocks approval when the entered amount or channel differs from the order', async () => {
  const wrapper = mount(ReviewPaymentDialog, { props: { payment } });
  await flushPromises();
  await wrapper.get('#actual-amount').setValue('0.02');
  await wrapper.get('#channel').setValue('wechat');
  await wrapper.get('#reference').setValue('MANUAL-REFERENCE');
  await wrapper.get('#confirmed').setValue(true);
  expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined();
  await wrapper.get('#actual-amount').setValue('0.01');
  await wrapper.get('#channel').setValue('bank');
  expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeDefined();
  wrapper.unmount();
});

it('opens an already reviewed receipt read-only instead of offering a second review', async () => {
  const wrapper = mount(ReviewPaymentDialog, { props: { payment: { ...payment, status: 'approved' } } });
  await flushPromises();
  expect(wrapper.get('#actual-amount').attributes('disabled')).toBeDefined();
  expect(wrapper.get('#reason').attributes('disabled')).toBeDefined();
  expect(wrapper.text()).toContain('不能再次提交');
  wrapper.unmount();
});

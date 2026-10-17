import { flushPromises, mount } from '@vue/test-utils';
import { createMemoryHistory, createRouter } from 'vue-router';
import { beforeEach, expect, it, vi } from 'vitest';
import Orders from './Orders.vue';

const { request } = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock('../session', () => ({ api: { request } }));
beforeEach(() => { request.mockReset(); request.mockResolvedValue({ total: 0, items: [], page: 1, page_size: 20 }); });
async function render(path = '/orders') {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/orders', component: Orders }, { path: '/:pathMatch(.*)*', component: { template: '<div />' } }] });
  await router.push(path); await router.isReady();
  const wrapper = mount(Orders, { global: { plugins: [router] } });
  await flushPromises(); return { wrapper, router };
}
it('uses the workbench status filter and searches order numbers or buyer email on the server', async () => {
  const { wrapper, router } = await render('/orders?status=fulfilling');
  expect(new URL(request.mock.calls[0][0], 'http://local').searchParams.get('status')).toBe('fulfilling');
  await wrapper.get('#order-q').setValue('buyer@example.test');
  await wrapper.get('form').trigger('submit'); await flushPromises();
  expect(router.currentRoute.value.query.q).toBe('buyer@example.test');
  const url = new URL(request.mock.calls.at(-1)![0], 'http://local');
  expect(url.searchParams.get('q')).toBe('buyer@example.test');
  expect(url.searchParams.get('status')).toBe('fulfilling');
  wrapper.unmount();
});
it('does not replace the current search with a late response from an earlier query', async () => {
  let resolveOld!: (value: unknown) => void;
  request.mockImplementationOnce(() => new Promise(resolve => { resolveOld = resolve; }));
  const { wrapper, router } = await render();
  await router.replace('/orders?q=new'); await flushPromises();
  resolveOld({ total: 1, items: [{ id: 'old', order_no: 'OLD', amount_cents: 1, status: 'completed', created_at: '2026-10-17T00:00:00Z', item: { product_title: '旧查询商品' } }], page: 1, page_size: 20 });
  await flushPromises();
  expect(wrapper.text()).not.toContain('旧查询商品');
  expect(wrapper.text()).toContain('没有符合条件的订单');
  wrapper.unmount();
});

it('retries the committed query without silently submitting a half-written search', async () => {
  request.mockRejectedValueOnce(new Error('暂时无法读取订单'));
  const { wrapper } = await render('/orders?q=committed');
  await wrapper.get('#order-q').setValue('still-editing');
  const retry = wrapper.findAll('button').find(button => button.text() === '重试')!;
  await retry.trigger('click'); await flushPromises();
  const url = new URL(request.mock.calls.at(-1)![0], 'http://local');
  expect(url.searchParams.get('q')).toBe('committed');
  wrapper.unmount();
});

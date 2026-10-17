import { flushPromises, mount } from '@vue/test-utils';
import { createMemoryHistory, createRouter } from 'vue-router';
import { beforeEach, expect, it, vi } from 'vitest';
import Workbench from './Workbench.vue';

const { request } = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock('../session', () => ({ api: { request } }));
const page = (total: number) => ({ total, items: [], page: 1, page_size: 5 });
beforeEach(() => {
  request.mockReset();
  request.mockImplementation(async (path: string) => {
    if (path.startsWith('/admin/payments')) return page(3);
    if (path.includes('status=fulfilling')) return page(2);
    if (path.startsWith('/admin/products')) return page(8);
    return page(12);
  });
});
async function render() {
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:pathMatch(.*)*', component: Workbench }] });
  await router.push('/'); await router.isReady();
  const wrapper = mount(Workbench, { global: { plugins: [router] } });
  await flushPromises(); return wrapper;
}
it('shows real task totals and links directly to their filtered queues', async () => {
  const wrapper = await render();
  const pending = wrapper.get('a[aria-label="查看待审核付款"]');
  expect(pending.text()).toContain('3');
  expect(pending.attributes('href')).toContain('/payments?status=pending');
  const delivering = wrapper.get('a[aria-label="查看待履约订单"]');
  expect(delivering.text()).toContain('2');
  expect(delivering.attributes('href')).toContain('/orders?status=fulfilling');
  expect(wrapper.text()).toContain('微信人工收款');
  wrapper.unmount();
});
it('keeps a failed task total unknown and supports retry without hiding other results', async () => {
  request.mockImplementation(async (path: string) => {
    if (path.startsWith('/admin/payments')) throw new Error('付款列表暂不可用');
    return page(2);
  });
  const wrapper = await render();
  expect(wrapper.get('a[aria-label="查看待审核付款"]').text()).toContain('暂不可用');
  expect(wrapper.get('a[aria-label="查看待履约订单"]').text()).toContain('2');
  request.mockResolvedValue(page(4));
  await wrapper.get('button').trigger('click'); await flushPromises();
  expect(wrapper.get('a[aria-label="查看待审核付款"]').text()).toContain('4');
  wrapper.unmount();
});

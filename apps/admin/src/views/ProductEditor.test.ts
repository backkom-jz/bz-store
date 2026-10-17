import { flushPromises, mount } from '@vue/test-utils';
import { createMemoryHistory, createRouter } from 'vue-router';
import { beforeEach, expect, it, vi } from 'vitest';
import ProductEditor from './ProductEditor.vue';
const { request } = vi.hoisted(() => ({ request: vi.fn() }));
vi.mock('../session', () => ({ api: { request } }));
beforeEach(() => { request.mockReset(); });

it('shows digital delivery guidance without offering an unrelated tutorial editor', async () => {
  request.mockImplementation(async (path: string) => {
    if (path === '/categories') return [];
    if (path.startsWith('/admin/tutorials')) return { items: [], total: 0, page: 1, page_size: 100 };
    return { id: 'digital-1', type: 'digital', title: '测试文件', slug: 'local-file', price_cents: 1, description: 'PDF', status: 'draft', category_id: null, is_demo: false };
  });
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/products/:id', name: 'product', component: ProductEditor }, { path: '/:pathMatch(.*)*', component: { template: '<div />' } }] });
  await router.push('/products/digital-1'); await router.isReady();
  const wrapper = mount(ProductEditor, { global: { plugins: [router] } }); await flushPromises();
  expect(wrapper.findAll('a').some(a => a.text() === '创建教程')).toBe(false);
  expect(wrapper.text()).toContain('数字商品发布步骤');
  expect(wrapper.text()).not.toContain('本阶段只售卖图文教程');
  expect(request.mock.calls.some(([path]) => path.startsWith('/admin/tutorials'))).toBe(false);
  wrapper.unmount();
});

it('retries only category loading and preserves the operator draft', async () => {
  let categoryAttempts = 0;
  request.mockImplementation(async (path: string) => {
    if (path === '/categories') {
      categoryAttempts++;
      if (categoryAttempts === 1) throw new Error('分类读取失败');
      return [];
    }
    return { id: 'digital-1', type: 'digital', title: '测试文件', slug: 'local-file', price_cents: 1, description: 'PDF', status: 'draft', category_id: null, is_demo: false };
  });
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/products/:id', name: 'product', component: ProductEditor }, { path: '/:pathMatch(.*)*', component: { template: '<div />' } }] });
  await router.push('/products/digital-1'); await router.isReady();
  const wrapper = mount(ProductEditor, { global: { plugins: [router] } }); await flushPromises();
  await wrapper.get('#title').setValue('运营人员已修改的标题');
  const retry = wrapper.findAll('button').find(button => button.text() === '重试读取分类');
  expect(retry).toBeDefined();
  await retry!.trigger('click'); await flushPromises();
  expect((wrapper.get('#title').element as HTMLInputElement).value).toBe('运营人员已修改的标题');
  expect(wrapper.get('button[type="submit"]').attributes('disabled')).toBeUndefined();
  expect(categoryAttempts).toBe(2);
  wrapper.unmount();
});

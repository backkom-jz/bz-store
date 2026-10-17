import { test, expect, type Page } from '@playwright/test';
import { mkdir } from 'node:fs/promises';

const created = '2026-10-17T08:30:00Z';
const user = { id: 'admin-local', email: 'operator@example.test', display_name: '本地运营', is_admin: true };
const product = { id: 'digital-1', type: 'digital', title: '微信一分钱验证文件', slug: 'local-one-cent', price_cents: 1, currency: 'CNY', description: '一分钱文件交付验证', status: 'draft', category_id: null, is_demo: true, created_at: created };
const order = { id: 'order-1', order_no: 'DS-LOCAL-001', user_id: 'buyer-1', user_email: 'buyer@example.test', amount_cents: 1, currency: 'CNY', status: 'payment_review', created_at: created, expires_at: created, paid_at: null, item: { id: 'item-1', product_id: product.id, product_title: product.title, product_type: 'digital', quantity: 1, unit_price_cents: 1, subtotal_cents: 1 }, payments: [{ id: 'proof-1', status: 'pending', review_note: null, created_at: created, reviewed_at: null }] };
const service = { ...order, id: 'service-1', order_no: 'DS-SERVICE-002', status: 'fulfilling', user_email: 'service@example.test', item: { ...order.item, product_title: '架构评审服务', product_type: 'service' } };
const payment = { id: 'proof-1', order_id: order.id, order_no: order.order_no, amount_cents: 1, payment_channel_expected: 'wechat', product_title: product.title, user_email: order.user_email, status: 'pending', created_at: created, reviewed_at: null, review_note: null, overdue: false, transaction_reference: null, actual_amount_cents: null, payment_channel: null };

async function fixtures(page: Page, failedTasks = false) {
  let approved = false;
  await page.route('**/api/v1/**', async route => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname.replace('/api/v1', '');
    let data: unknown;
    if (path === '/users/me') data = user;
    else if (path === '/auth/csrf') data = { csrf_token: 'local-fixture-csrf' };
    else if (path === '/admin/payments/proof-1/proof') {
      await route.fulfill({ status: 200, contentType: 'image/png', body: Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==', 'base64') }); return;
    } else if (path === '/admin/payments/proof-1/approve') {
      expect(request.postDataJSON()).toEqual({ actual_amount_cents: 1, payment_channel: 'wechat', transaction_reference: 'WECHAT-ONE-CENT-LOCAL', receipt_confirmed: true });
      approved = true; data = { ...order, status: 'completed' };
    } else if (path === '/admin/payments') {
      if (failedTasks) { await route.fulfill({ status: 503, json: { code: 'UNAVAILABLE', message: '付款待办读取失败', data: null, request_id: 'local' } }); return; }
      const items = approved ? [] : [payment]; data = { items, total: items.length, page: 1, page_size: 20 };
    } else if (path === '/admin/orders') {
      const status = url.searchParams.get('status'); const q = (url.searchParams.get('q') || '').toLowerCase();
      const items = [order, service].filter(item => (!status || item.status === status) && (!q || [item.order_no, item.user_email, item.item.product_title].some(value => value.toLowerCase().includes(q))));
      data = { items, total: items.length, page: 1, page_size: 20 };
    } else if (path === '/admin/products') data = { items: [product], total: 1, page: 1, page_size: 20 };
    else if (path === '/admin/products/digital-1') data = product;
    else if (path === '/categories') data = [];
    else if (path === '/admin/orders/order-1') data = order;
    else { await route.fulfill({ status: 404, json: { code: 'NOT_FOUND', message: '本地夹具未实现此请求', data: null, request_id: 'local' } }); return; }
    await route.fulfill({ json: { code: 'OK', message: 'ok', data, request_id: 'local' } });
  });
}

test('workbench exposes actionable tasks and fits desktop and mobile screens', async ({ page }, testInfo) => {
  await fixtures(page);
  await page.goto('/admin/');
  await expect(page.getByRole('heading', { name: '运营工作台' })).toBeVisible();
  await expect(page.getByRole('link', { name: '查看待审核付款' })).toContainText('1');
  await expect(page.getByRole('link', { name: '查看待履约订单' })).toContainText('1');
  await expect(page.getByRole('navigation', { name: '后台导航' })).toContainText('商品管理');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
  await mkdir('.runtime', { recursive: true });
  await page.screenshot({ path: `.runtime/admin-workbench-${testInfo.project.name}.png`, fullPage: true });
  await page.getByRole('link', { name: '查看待履约订单' }).click();
  await expect(page).toHaveURL(/status=fulfilling/);
  await expect(page.getByText('架构评审服务', { exact: true })).toBeVisible();
  await page.getByLabel('订单号 / 买家邮箱 / 商品名称').fill('no-match');
  await page.getByRole('button', { name: '查询订单' }).click();
  await expect(page.getByText('没有符合条件的订单')).toBeVisible();
});

test('one-cent WeChat review requires receipt confirmation and a matching amount', async ({ page }) => {
  await fixtures(page);
  await page.goto('/admin/payments?status=pending');
  await page.getByRole('button', { name: '审核', exact: true }).click();
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByLabel('实际到账金额（元）').fill('0.02');
  await page.getByLabel('渠道交易流水号').fill('WECHAT-ONE-CENT-LOCAL');
  await page.getByLabel('已核实真实收款记录').check();
  await expect(page.getByRole('button', { name: '确认到账并开通' })).toBeDisabled();
  await page.getByLabel('实际到账金额（元）').fill('0.01');
  await page.getByRole('button', { name: '确认到账并开通' }).click();
  await expect(page.getByRole('status').filter({ hasText: '已核实到账' })).toBeVisible();
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('digital editor explains publication and failed workbench totals remain unknown', async ({ page }) => {
  await fixtures(page, true);
  await page.goto('/admin/workbench');
  await expect(page.getByRole('link', { name: '查看待审核付款' })).toContainText('暂不可用');
  await expect(page.getByRole('link', { name: '查看待履约订单' })).toContainText('1');
  await page.goto('/admin/products/digital-1');
  await expect(page.getByRole('heading', { name: '数字商品发布步骤' })).toBeVisible();
  await expect(page.getByRole('link', { name: '创建教程', exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
});

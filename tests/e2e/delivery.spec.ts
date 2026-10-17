import type { Page } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { test, expect, adminApi, hydrated, origin, receiptPng, verifyMailbox } from './support';

const pdf = Buffer.from('%PDF-1.4\nDevStore browser delivery fixture\n%%EOF\n');

async function loginBuyer(page: Page) {
  const email = `delivery-${crypto.randomUUID()}@example.test`;
  const password = 'Browser-delivery-test-42!';
  await page.goto('/register');
  await hydrated(page);
  await page.getByLabel('邮箱').fill(email);
  await page.getByLabel('密码', { exact: true }).fill(password);
  await page.getByRole('button', { name: '创建账号' }).click();
  await expect(page.getByRole('status')).toContainText('请检查你的邮箱');
  await verifyMailbox(email, (path, data) => page.request.post(origin + path, { data, headers: { Origin: origin } }));
  await page.goto('/login');
  await hydrated(page);
  await page.getByLabel('邮箱').fill(email);
  await page.getByLabel('密码', { exact: true }).fill(password);
  await page.getByRole('button', { name: '登录', exact: true }).click();
  await expect(page).toHaveURL(origin + '/');
}

async function createProduct(admin: Awaited<ReturnType<typeof adminApi>>, kind: string, price: number) {
  const slug = `delivery-${crypto.randomUUID()}`;
  const product = await admin.write('post', '/admin/products', { title: '浏览器交付回归商品', slug, type: kind, price_cents: price });
  let fileId = '';
  if (kind === 'digital') {
    const csrf = (await (await admin.api.get('/api/v1/auth/csrf')).json()).data.csrf_token;
    const uploaded = await admin.api.post(`/api/v1/admin/products/${product.id}/file`, {
      headers: { 'X-CSRF-Token': csrf },
      multipart: { file: { name: 'private.pdf', mimeType: 'application/pdf', buffer: pdf } },
    });
    expect(uploaded.status()).toBe(201);
    fileId = (await uploaded.json()).data.file_id;
  }
  await admin.write('patch', `/admin/products/${product.id}`, { status: 'published' });
  return { ...product, slug, fileId };
}

test('digital payment grants a private download and a refund revokes it', async ({ page }) => {
  test.setTimeout(180000);
  const admin = await adminApi();
  try {
    const product = await createProduct(admin, 'digital', 9900);
    expect((await page.request.get(`/api/v1/me/downloads/${product.fileId}`)).status()).toBe(401);
    await loginBuyer(page);
    expect((await page.request.get(`/api/v1/me/downloads/${product.fileId}`)).status()).toBe(404);
    await page.goto('/products/' + product.slug);
    await hydrated(page);
    await page.getByRole('button', { name: '立即购买' }).click();
    await expect(page).toHaveURL(/\/checkout\//);
    const orderId = new URL(page.url()).pathname.split('/').pop();
    await page.setInputFiles('#receipt', { name: 'receipt.png', mimeType: 'image/png', buffer: receiptPng });
    await page.getByRole('button', { name: '提交付款凭证' }).click();
    await expect(page.getByRole('heading', { name: '凭证已提交，等待核实到账' })).toBeVisible();
    const order = (await (await page.request.get('/api/v1/orders/' + orderId)).json()).data;
    await admin.write('post', `/admin/payments/${order.payments[0].id}/approve`, {
      actual_amount_cents: 9900, payment_channel: 'bank', transaction_reference: `delivery-${crypto.randomUUID()}`, receipt_confirmed: true,
    });
    await page.goto('/user/library');
    const card = page.locator('article').filter({ has: page.getByRole('heading', { name: product.title }) });
    const downloading = page.waitForEvent('download');
    await card.getByRole('button', { name: '下载文件' }).click();
    const download = await downloading;
    const path = await download.path();
    expect(path).toBeTruthy();
    expect(await readFile(path!)).toEqual(pdf);
    await admin.write('post', `/admin/orders/${orderId}/refund`, {
      amount_cents: 9900, reference_no: `refund-${crypto.randomUUID()}`, note: '浏览器测试整单人工退款',
    });
    expect((await page.request.get(`/api/v1/me/downloads/${product.fileId}`)).status()).toBe(404);
    await page.reload();
    await expect(page.getByText('还没有已购内容')).toBeVisible();
    const refunded = (await (await page.request.get('/api/v1/orders/' + orderId)).json()).data;
    expect(refunded.status).toBe('refunded');
  } finally {
    await admin.api.dispose();
  }
});

test('service buyer can communicate and accept delivery from their ticket', async ({ page }) => {
  test.setTimeout(180000);
  const admin = await adminApi();
  try {
    const product = await createProduct(admin, 'service', 0);
    await loginBuyer(page);
    await page.goto('/products/' + product.slug);
    await hydrated(page);
    await page.getByRole('button', { name: '免费领取' }).click();
    await expect(page).toHaveURL(/\/checkout\//);
    const orderId = new URL(page.url()).pathname.split('/').pop();
    const tickets = (await (await page.request.get('/api/v1/me/tickets')).json()).data.items;
    expect(tickets).toHaveLength(1);
    const ticketId = tickets[0].id;
    await page.goto('/user/tickets/' + ticketId);
    await page.getByLabel('留言').fill('请核对约定的交付内容');
    await page.getByRole('button', { name: '发送', exact: true }).click();
    await expect(page.getByText('请核对约定的交付内容')).toBeVisible();
    await admin.write('post', `/admin/tickets/${ticketId}/messages`, { body: '服务交付已完成，请验收' });
    await page.reload();
    await expect(page.getByText('服务交付已完成，请验收')).toBeVisible();
    await page.getByRole('button', { name: '确认交付完成' }).click();
    await expect(page.getByRole('button', { name: '确认交付完成' })).toHaveCount(0);
    expect((await (await page.request.get('/api/v1/orders/' + orderId)).json()).data.status).toBe('completed');
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1)).toBe(true);
  } finally {
    await admin.api.dispose();
  }
});

test('blog draft is private and published markdown can be read publicly', async ({ page }) => {
  test.setTimeout(180000);
  const admin = await adminApi();
  try {
    const slug = `article-${crypto.randomUUID()}`;
    const body = { title: '浏览器文章发布回归', slug, excerpt: '用于验证草稿与公开正文', content_md: '# 公开文章正文\n\n发布后可以阅读。', status: 'draft' };
    const article = await admin.write('post', '/admin/blog', body);
    expect((await page.request.get('/api/v1/blog/' + slug)).status()).toBe(404);
    await admin.write('patch', '/admin/blog/' + article.id, { ...body, status: 'published' });
    await page.goto('/blog/' + slug);
    await expect(page.getByRole('heading', { name: body.title })).toBeVisible();
    await expect(page.getByRole('heading', { name: '公开文章正文' })).toBeVisible();
    await admin.write('patch', '/admin/blog/' + article.id, body);
    expect((await page.request.get('/api/v1/blog/' + slug)).status()).toBe(404);
  } finally {
    await admin.api.dispose();
  }
});

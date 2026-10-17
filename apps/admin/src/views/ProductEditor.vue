<script setup lang="ts">
import type { Category, Page, ProductStatus, ProductSummary, TutorialSummary } from '@devstore/shared-types';
import { money, parseCents } from '@devstore/ui/format';
import { computed, ref, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import { centsToYuanInput, slugError } from '../catalog';
import { api } from '../session';

const route = useRoute();
const router = useRouter();
const creating = computed(() => route.name === 'product-new');
const product = ref<ProductSummary | null>(null);
const categories = ref<Category[]>([]);
const tutorialId = ref<string | null>(null);
const title = ref('');
const slug = ref('');
const kind = ref<'tutorial' | 'digital' | 'service'>('tutorial');
const price = ref('');
const description = ref('');
const status = ref<ProductStatus>('draft');
const categoryId = ref('');
const pending = ref(false);
const saving = ref(false);
const uploading = ref(false);
const error = ref('');
const notice = ref('');
const categoryError = ref('');
const categoryLoading = ref(false);
const fileAttached = ref(false);
const kindLabel = computed(() => ({ tutorial: '教程', digital: '数字商品', service: '服务' })[kind.value]);
const publishSteps = computed(() => ({
  tutorial: ['保存商品草稿，设置价格与介绍', '创建教程，编辑并发布章节；付费教程至少有一章非预览内容', '发布教程后，将商品状态设为已发布并保存'],
  digital: ['保存商品草稿，设置价格与介绍', '上传 5 MiB 以内的 PDF 或 ZIP 交付文件', '将商品状态设为已发布并保存；售出后不可替换文件'],
  service: ['保存商品草稿，写清服务范围与交付说明', '将商品状态设为已发布并保存', '核实收款后，在服务工单中沟通、交付并等待用户验收'],
})[kind.value]);

const priceHint = computed(() => {
  if (!price.value.trim()) return '请输入售价，0 元表示免费领取。';
  try {
    return `售卖价：${money(parseCents(price.value.trim()))}，用户下单时将按此金额付款。`;
  } catch (reason) {
    return reason instanceof Error ? reason.message : '金额无效';
  }
});

async function findTutorial(productId: string) {
  for (let page = 1; page <= 20; page += 1) {
    const data = await api.request<Page<TutorialSummary>>('/admin/tutorials?page=' + page + '&page_size=100');
    const found = data.items.find((item) => item.product_id === productId);
    if (found) return found.id;
    if (page * data.page_size >= data.total) return null;
  }
  return null;
}

async function loadCategories() {
  categoryLoading.value = true;
  categoryError.value = '';
  try {
    categories.value = await api.request<Category[]>('/categories');
  } catch {
    categoryError.value = '分类暂时读取失败。请重试读取分类，已填写的商品内容会保留。';
  } finally {
    categoryLoading.value = false;
  }
}

async function load() {
  error.value = '';
  notice.value = '';
  tutorialId.value = null;
  categoryError.value = '';
  fileAttached.value = false;
  product.value = null;
  pending.value = true;
  try {
    await loadCategories();
    if (creating.value) {
      title.value = '';
      slug.value = '';
      kind.value = 'tutorial';
      price.value = '';
      description.value = '';
      status.value = 'draft';
      categoryId.value = '';
      return;
    }
    const row = await api.request<ProductSummary>('/admin/products/' + route.params.id);
    product.value = row;
    title.value = row.title;
    slug.value = row.slug;
    kind.value = row.type;
    price.value = centsToYuanInput(row.price_cents);
    description.value = row.description;
    status.value = row.status;
    categoryId.value = row.category_id ?? '';
    if (row.type === 'tutorial') tutorialId.value = await findTutorial(row.id);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

async function save() {
  if (saving.value || uploading.value || categoryLoading.value || categoryError.value) return;
  const problem = slugError(slug.value);
  if (problem) {
    error.value = problem;
    return;
  }
  let priceCents = 0;
  try {
    priceCents = parseCents(price.value.trim());
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '金额无效';
    return;
  }
  if (!title.value.trim()) {
    error.value = '请填写商品标题';
    return;
  }
  saving.value = true;
  error.value = '';
  notice.value = '';
  const body = {
    title: title.value.trim(),
    slug: slug.value.trim(),
    type: kind.value,
    price_cents: priceCents,
    description: description.value,
    status: creating.value ? 'draft' : status.value,
    category_id: categoryId.value || null,
  };
  try {
    const saved = await api.request<ProductSummary>(creating.value ? '/admin/products' : '/admin/products/' + route.params.id, {
      method: creating.value ? 'POST' : 'PATCH',
      body,
    });
    notice.value = '商品已保存。';
    if (creating.value) {
      await router.replace('/products/' + saved.id);
      return;
    }
    product.value = saved;
    status.value = saved.status;
    price.value = centsToYuanInput(saved.price_cents);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '保存失败，请重试';
  } finally {
    saving.value = false;
  }
}

async function uploadFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = '';
  if (!file || !product.value) return;
  uploading.value = true;
  error.value = '';
  notice.value = '';
  const body = new FormData();
  body.append('file', file);
  try {
    await api.request('/admin/products/' + product.value.id + '/file', { method: 'POST', body });
    fileAttached.value = true;
    notice.value = '交付文件已上传。现在可以将商品状态设为已发布并保存。';
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '上传失败，请重试';
  } finally {
    uploading.value = false;
  }
}

watch(() => route.fullPath, () => void load(), { immediate: true });
</script>

<template>
  <p><RouterLink class="text-link" to="/products">返回商品列表</RouterLink></p>
  <header class="page-head">
    <h1>{{ creating ? '新建商品' : '编辑商品' }}</h1>
    <p class="muted">按商品类型完成内容或文件准备，再发布供用户购买。</p>
  </header>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error && !product && !creating" class="error" role="alert">
    <p>{{ error }}</p>
    <button class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <form v-else class="panel editor" @submit.prevent="save">
    <section class="publish-guide" aria-label="商品发布指引"><span class="badge">{{ kindLabel }}</span><h2>{{ kindLabel }}发布步骤</h2><ol><li v-for="step in publishSteps" :key="step">{{ step }}</li></ol><p v-if="creating" class="hint">第一次保存会创建草稿，随后继续准备内容和发布。</p></section>
    <p v-if="product?.is_demo" class="badge amber">示例内容</p>
    <div class="field">
      <label for="title">标题</label>
      <input id="title" v-model="title" maxlength="200" required :disabled="saving">
    </div>
    <div class="field">
      <label for="slug">访问标识（slug）</label>
      <input id="slug" v-model="slug" maxlength="220" required autocomplete="off" spellcheck="false" :disabled="saving">
      <p class="hint">小写字母、数字和单连字符，例如 rust-axum-starter。</p>
    </div>
    <div class="field">
      <label for="kind">类型</label>
      <select id="kind" v-model="kind" :disabled="saving || !creating">
        <option value="tutorial">教程</option>
        <option value="digital">数字商品</option>
        <option value="service">服务</option>
      </select>
      <p class="hint">创建后不能改类型。教程需要已发布章节，数字商品需要 PDF 或 ZIP，服务在付款后进入工单。</p>
    </div>
    <div v-if="product && kind === 'digital'" class="field">
      <label for="asset">交付文件</label>
      <input id="asset" type="file" accept="application/pdf,application/zip,.pdf,.zip" :disabled="uploading || saving" @change="uploadFile">
      <p class="hint">仅支持 5 MiB 以内的 PDF 或 ZIP。已有订单后不能更换。</p><p v-if="uploading" role="status" class="notice">正在上传文件，请等待完成后再保存。</p><p v-else-if="fileAttached" class="notice" role="status">本次文件上传成功。</p>
    </div>
    <div class="field">
      <label for="price">价格（元）</label>
      <input id="price" v-model="price" inputmode="decimal" autocomplete="off" required placeholder="例如 0.01" :disabled="saving">
      <p class="hint">{{ priceHint }} 微信一分钱流程验证可填写 0.01，不要修改正式商品的原价。</p>
    </div>
    <div class="field">
      <label for="category">分类</label>
      <select id="category" v-model="categoryId" :disabled="saving || categoryLoading || !!categoryError">
        <option value="">不分类</option>
        <option v-for="category in categories" :key="category.id" :value="category.id">{{ category.name }}</option>
      </select>
    </div>
    <div class="field">
      <label for="description">商品介绍与交付范围</label>
      <textarea id="description" v-model="description" maxlength="20000" :disabled="saving"></textarea>
    </div>
    <div v-if="!creating" class="field">
      <label for="status">状态</label>
      <select id="status" v-model="status" :disabled="saving">
        <option value="draft">草稿</option>
        <option value="published">已发布</option>
        <option value="archived">已下架</option>
      </select>
      <p class="hint">教程需先发布章节；数字商品需先上传文件；服务可以直接发布。</p>
    </div>
    <div v-if="categoryError" class="error" role="alert"><p>{{ categoryError }}</p><button class="btn secondary small" type="button" :disabled="categoryLoading" @click="loadCategories">重试读取分类</button></div>
    <p v-if="notice" class="notice" role="status">{{ notice }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <div class="form-actions">
      <button class="btn" type="submit" :disabled="saving || uploading || categoryLoading || !!categoryError">{{ saving ? '正在保存…' : creating ? '保存草稿并继续' : '保存商品' }}</button>
      <RouterLink v-if="kind === 'tutorial' && tutorialId" class="btn secondary" :to="'/tutorials/' + tutorialId">编辑教程章节</RouterLink>
      <RouterLink v-else-if="product && kind === 'tutorial'" class="btn secondary" :to="{ path: '/tutorials/new', query: { product: product.id } }">创建教程</RouterLink>
    </div>
  </form>
</template>

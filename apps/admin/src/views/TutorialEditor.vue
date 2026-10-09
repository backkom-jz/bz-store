<script setup lang="ts">
import type { ChapterSummary, ProductSummary, TutorialDetail } from '@devstore/shared-types';
import { statusLabel } from '@devstore/ui/format';
import { ApiError } from '@devstore/api-client';
import { computed, ref, watch } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import { blankChapter, parseSortOrder, slugError, type ChapterDraft } from '../catalog';
import { api } from '../session';

const route = useRoute();
const router = useRouter();
const creating = computed(() => route.name === 'tutorial-new');
const productId = computed(() => typeof route.query.product === 'string' ? route.query.product : '');
const tutorial = ref<TutorialDetail | null>(null);
const product = ref<ProductSummary | null>(null);
const title = ref('');
const slug = ref('');
const description = ref('');
const status = ref<'draft' | 'published'>('draft');
const drafts = ref<Record<string, ChapterDraft>>({});
const fresh = ref<ChapterDraft>(blankChapter());
const pending = ref(false);
const saving = ref('');
const error = ref('');
const notice = ref('');

function draftFrom(chapter: ChapterSummary): ChapterDraft {
  return {
    title: chapter.title,
    content_md: chapter.content_md ?? '',
    is_preview: chapter.is_preview,
    sort_order: String(chapter.sort_order),
    status: chapter.status === 'published' ? 'published' : 'draft',
  };
}

function applyTutorial(row: TutorialDetail) {
  drafts.value = Object.fromEntries(row.chapters.map((chapter) => [chapter.id, draftFrom(chapter)]));
  tutorial.value = row;
  title.value = row.title;
  slug.value = row.slug;
  description.value = row.description;
  status.value = row.status;
  const next = row.chapters.reduce((max, chapter) => Math.max(max, chapter.sort_order + 1), 0);
  fresh.value = blankChapter(next);
}

async function load() {
  error.value = '';
  notice.value = '';
  tutorial.value = null;
  product.value = null;
  if (creating.value && !productId.value) {
    error.value = '请从商品页创建教程。';
    return;
  }
  pending.value = true;
  try {
    if (creating.value) {
      product.value = await api.request<ProductSummary>('/admin/products/' + productId.value);
      title.value = product.value.title;
      slug.value = product.value.slug;
      description.value = product.value.description;
      status.value = 'draft';
      fresh.value = blankChapter();
      return;
    }
    applyTutorial(await api.request<TutorialDetail>('/admin/tutorials/' + route.params.id));
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

async function saveTutorial() {
  const problem = slugError(slug.value);
  if (problem) {
    error.value = problem;
    return;
  }
  if (!title.value.trim()) {
    error.value = '请填写教程标题';
    return;
  }
  const linked = creating.value ? productId.value : tutorial.value?.product_id;
  if (!linked) {
    error.value = '缺少关联商品';
    return;
  }
  saving.value = 'tutorial';
  error.value = '';
  notice.value = '';
  const body = {
    product_id: linked,
    title: title.value.trim(),
    slug: slug.value.trim(),
    description: description.value,
    status: creating.value ? 'draft' : status.value,
  };
  try {
    const saved = await api.request<TutorialDetail>(creating.value ? '/admin/tutorials' : '/admin/tutorials/' + route.params.id, {
      method: creating.value ? 'POST' : 'PATCH',
      body,
    });
    notice.value = '教程已保存。';
    if (creating.value) {
      await router.replace('/tutorials/' + saved.id);
      return;
    }
    applyTutorial(saved);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '保存失败，请重试';
  } finally {
    saving.value = '';
  }
}

function chapterBody(draft: ChapterDraft) {
  const sort = parseSortOrder(draft.sort_order);
  if (!draft.title.trim()) throw new Error('请填写章节标题');
  if (sort === null) throw new Error('排序必须是非负整数');
  if (draft.content_md.length > 500_000) throw new Error('章节正文过长');
  return {
    title: draft.title.trim(),
    content_md: draft.content_md,
    is_preview: draft.is_preview,
    sort_order: sort,
    status: draft.status,
  };
}

async function saveChapter(id: string | null) {
  if (!tutorial.value || saving.value) return;
  const draft = id ? drafts.value[id] : fresh.value;
  if (!draft) return;
  let body;
  try {
    body = chapterBody(draft);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '章节内容无效';
    return;
  }
  const tutorialId = tutorial.value.id;
  const kept = { ...fresh.value };
  const edited = { ...draft };
  saving.value = id ?? 'new';
  error.value = '';
  notice.value = '';
  try {
    await api.request(id ? '/admin/tutorials/' + tutorialId + '/chapters/' + id : '/admin/tutorials/' + tutorialId + '/chapters', {
      method: id ? 'PATCH' : 'POST',
      body,
    });
    notice.value = id ? '章节已保存。' : body.is_preview ? '章节已添加，未购买的人可以阅读这一章。' : '章节已添加。未勾选预览，只有购买后才能阅读。';
    applyTutorial(await api.request<TutorialDetail>('/admin/tutorials/' + tutorialId));
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '章节保存失败，请重试';
    if (reason instanceof ApiError) {
      try {
        applyTutorial(await api.request<TutorialDetail>('/admin/tutorials/' + tutorialId));
      } catch { /* keep the save error */ }
      if (!id) fresh.value = kept;
      else drafts.value[id] = edited;
    }
  } finally {
    saving.value = '';
  }
}

watch(() => route.fullPath, () => void load(), { immediate: true });
</script>

<template>
  <p class="page-links">
    <RouterLink class="text-link" to="/tutorials">返回教程列表</RouterLink>
    <RouterLink v-if="tutorial" class="text-link" :to="'/products/' + tutorial.product_id">查看商品</RouterLink>
    <RouterLink v-else-if="product" class="text-link" :to="'/products/' + product.id">查看商品</RouterLink>
  </p>
  <header class="page-head">
    <h1>{{ creating ? '新建教程' : '编辑教程' }}</h1>
    <p class="muted">预览章节会向未购买的人公开。新章节默认不勾选。</p>
  </header>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <div v-else-if="error && !tutorial && !product" class="error" role="alert">
    <p>{{ error }}</p>
    <button v-if="!creating || productId" class="btn secondary small" type="button" @click="load">重试</button>
  </div>
  <div v-else class="stack">
    <form class="panel editor" @submit.prevent="saveTutorial">
      <div class="field">
        <label for="tutorial-title">标题</label>
        <input id="tutorial-title" v-model="title" maxlength="200" required :disabled="!!saving">
      </div>
      <div class="field">
        <label for="tutorial-slug">slug</label>
        <input id="tutorial-slug" v-model="slug" maxlength="220" required autocomplete="off" spellcheck="false" :disabled="!!saving">
      </div>
      <div class="field">
        <label for="tutorial-description">介绍</label>
        <textarea id="tutorial-description" v-model="description" maxlength="20000" :disabled="!!saving"></textarea>
      </div>
      <div v-if="!creating" class="field">
        <label for="tutorial-status">状态</label>
        <select id="tutorial-status" v-model="status" :disabled="!!saving">
          <option value="draft">草稿</option>
          <option value="published">已发布</option>
        </select>
      </div>
      <div class="form-actions">
        <button class="btn" type="submit" :disabled="!!saving">{{ saving === 'tutorial' ? '正在保存…' : '保存教程' }}</button>
      </div>
    </form>

    <section v-if="tutorial" class="panel editor">
      <h2>章节</h2>
      <article v-for="chapter in tutorial.chapters" :key="chapter.id" class="chapter">
        <p class="muted hint">当前：{{ statusLabel[chapter.status || 'draft'] }}<template v-if="chapter.is_preview"> · 预览公开</template></p>
        <form @submit.prevent="saveChapter(chapter.id)">
          <div class="field">
            <label :for="'chapter-title-' + chapter.id">章节标题</label>
            <input :id="'chapter-title-' + chapter.id" v-model="drafts[chapter.id].title" maxlength="200" :disabled="!!saving">
          </div>
          <div class="field">
            <label :for="'chapter-body-' + chapter.id">正文</label>
            <textarea :id="'chapter-body-' + chapter.id" v-model="drafts[chapter.id].content_md" :disabled="!!saving"></textarea>
          </div>
          <div class="field narrow">
            <label :for="'chapter-order-' + chapter.id">排序</label>
            <input :id="'chapter-order-' + chapter.id" v-model="drafts[chapter.id].sort_order" inputmode="numeric" :disabled="!!saving">
          </div>
          <div class="field">
            <label :for="'chapter-status-' + chapter.id">章节状态</label>
            <select :id="'chapter-status-' + chapter.id" v-model="drafts[chapter.id].status" :disabled="!!saving">
              <option value="draft">草稿</option>
              <option value="published">已发布</option>
            </select>
          </div>
          <label class="check" :for="'chapter-preview-' + chapter.id">
            <input :id="'chapter-preview-' + chapter.id" v-model="drafts[chapter.id].is_preview" type="checkbox" :disabled="!!saving">
            公开预览。未勾选时，只有购买后才能阅读。
          </label>
          <button class="btn secondary small" type="submit" :disabled="!!saving">{{ saving === chapter.id ? '正在保存…' : '保存章节' }}</button>
        </form>
      </article>

      <form class="chapter" @submit.prevent="saveChapter(null)">
        <h2>添加章节</h2>
        <div class="field">
          <label for="new-title">章节标题</label>
          <input id="new-title" v-model="fresh.title" maxlength="200" :disabled="!!saving">
        </div>
        <div class="field">
          <label for="new-body">正文</label>
          <textarea id="new-body" v-model="fresh.content_md" :disabled="!!saving"></textarea>
        </div>
        <div class="field narrow">
          <label for="new-order">排序</label>
          <input id="new-order" v-model="fresh.sort_order" inputmode="numeric" :disabled="!!saving">
        </div>
        <div class="field">
          <label for="new-status">章节状态</label>
          <select id="new-status" v-model="fresh.status" :disabled="!!saving">
            <option value="draft">草稿</option>
            <option value="published">已发布</option>
          </select>
        </div>
        <label class="check" for="new-preview">
          <input id="new-preview" v-model="fresh.is_preview" type="checkbox" :disabled="!!saving">
          公开预览。默认不勾选。
        </label>
        <button class="btn" type="submit" :disabled="!!saving">{{ saving === 'new' ? '正在添加…' : '添加章节' }}</button>
      </form>
    </section>
    <p v-if="notice" class="notice" role="status">{{ notice }}</p>
    <p v-if="error && (tutorial || product)" class="error" role="alert">{{ error }}</p>
  </div>
</template>

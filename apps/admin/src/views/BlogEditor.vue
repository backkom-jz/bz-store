<script setup lang="ts">
import type { BlogPost } from '@devstore/shared-types';
import { computed, onMounted, ref } from 'vue';
import { RouterLink, useRoute, useRouter } from 'vue-router';
import { api } from '../session';

const route = useRoute();
const router = useRouter();
const creating = computed(() => route.name === 'blog-new');
const title = ref('');
const slug = ref('');
const excerpt = ref('');
const content = ref('');
const status = ref<'draft' | 'published'>('draft');
const error = ref('');
const notice = ref('');
const pending = ref(false);
const saving = ref(false);

async function load() {
  if (creating.value) return;
  pending.value = true;
  error.value = '';
  try {
    const post = await api.request<BlogPost>('/admin/blog/' + route.params.id);
    title.value = post.title;
    slug.value = post.slug;
    excerpt.value = post.excerpt;
    content.value = post.content_md || '';
    status.value = post.status || 'draft';
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
  } finally {
    pending.value = false;
  }
}

async function save() {
  saving.value = true;
  error.value = '';
  notice.value = '';
  const body = { title: title.value.trim(), slug: slug.value.trim(), excerpt: excerpt.value.trim(), content_md: content.value, status: status.value };
  try {
    const saved = await api.request<BlogPost>(creating.value ? '/admin/blog' : '/admin/blog/' + route.params.id, {
      method: creating.value ? 'POST' : 'PATCH',
      body,
    });
    notice.value = '文章已保存。';
    if (creating.value) await router.replace('/blog/' + saved.id);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : '保存失败，请重试';
  } finally {
    saving.value = false;
  }
}

onMounted(() => void load());
</script>

<template>
  <p><RouterLink class="text-link" to="/blog">返回博客</RouterLink></p>
  <p v-if="pending" class="muted" role="status">正在加载…</p>
  <form v-else class="panel editor" @submit.prevent="save">
    <div class="field"><label for="title">标题</label><input id="title" v-model="title" maxlength="200" required :disabled="saving"></div>
    <div class="field"><label for="slug">slug</label><input id="slug" v-model="slug" maxlength="220" required :disabled="saving"></div>
    <div class="field"><label for="excerpt">摘要</label><textarea id="excerpt" v-model="excerpt" maxlength="500" :disabled="saving"></textarea></div>
    <div class="field"><label for="content">正文</label><textarea id="content" v-model="content" maxlength="100000" required :disabled="saving"></textarea><p class="hint">使用 Markdown，不能包含 HTML 标签。</p></div>
    <div class="field"><label for="status">状态</label><select id="status" v-model="status" :disabled="saving"><option value="draft">草稿</option><option value="published">已发布</option></select></div>
    <p v-if="notice" class="notice" role="status">{{ notice }}</p>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <button class="btn" type="submit" :disabled="saving">{{ saving ? '正在保存…' : '保存文章' }}</button>
  </form>
</template>

import type { Page } from '@devstore/shared-types';
import { computed, onBeforeUnmount, ref, shallowRef, watch } from 'vue';
import { useRoute, useRouter } from 'vue-router';
import { api } from '../session';

export function useAdminList<T>(endpoint: string, defaultStatus = '') {
  const route = useRoute();
  const router = useRouter();
  const q = ref('');
  const status = ref(defaultStatus);
  const data = shallowRef<Page<T> | null>(null);
  const pending = ref(false);
  const error = ref('');
  let revision = 0;
  let controller: AbortController | undefined;
  const page = computed(() => {
    const parsed = Number(route.query.page ?? 1);
    return Number.isSafeInteger(parsed) && parsed >= 1 && parsed <= 1_000_000 ? parsed : 1;
  });
  function readFilters() {
    q.value = typeof route.query.q === 'string' ? route.query.q : '';
    status.value = route.query.status === 'all' ? '' : typeof route.query.status === 'string' ? route.query.status : defaultStatus;
  }
  async function load() {
    const current = ++revision;
    controller?.abort();
    controller = new AbortController();
    pending.value = true; error.value = '';
    const params = new URLSearchParams({ page: String(page.value), page_size: '20' });
    const committedQ = typeof route.query.q === 'string' ? route.query.q.trim() : '';
    const committedStatus = route.query.status === 'all' ? '' : typeof route.query.status === 'string' ? route.query.status : defaultStatus;
    if (committedQ) params.set('q', committedQ);
    if (committedStatus) params.set('status', committedStatus);
    try {
      const result = await api.request<Page<T>>(endpoint + '?' + params, { signal: controller.signal });
      if (revision === current) data.value = result;
    } catch (reason) {
      if (revision === current) error.value = reason instanceof Error ? reason.message : '加载失败，请重试';
    } finally {
      if (revision === current) pending.value = false;
    }
  }
  async function search() {
    const target = { query: { q: q.value.trim() || undefined, status: status.value || 'all' } };
    if (router.resolve(target).fullPath === route.fullPath) await load();
    else await router.replace(target);
  }
  async function goPage(value: number) {
    await router.replace({ query: { ...route.query, page: value > 1 ? value : undefined } });
  }
  async function reset() {
    q.value = ''; status.value = defaultStatus;
    await search();
  }
  watch(() => route.fullPath, () => { readFilters(); void load(); }, { immediate: true });
  onBeforeUnmount(() => { revision++; controller?.abort(); });
  return { q, status, data, pending, error, page, load, search, reset, goPage };
}

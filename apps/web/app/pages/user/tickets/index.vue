<script setup lang="ts">
import type { Page, Ticket } from '@devstore/shared-types';
import { dateTime, statusLabel } from '@devstore/ui/format';

definePageMeta({ middleware: 'auth' });
useSeoMeta({ title: '我的工单 · DevStore' });
const api = useApi();
const page = ref(1);
const { data, error, status, refresh } = await useAsyncData(
  () => 'private:tickets:' + page.value,
  () => api.request<Page<Ticket>>('/me/tickets?page=' + page.value),
  { server: false },
);
</script>

<template>
  <div class="container">
    <div class="page-head"><h1>我的工单</h1><p class="muted">服务商品付款后在这里沟通交付。</p></div>
    <UserNav />
    <ClientOnly>
      <ApiFeedback :pending="status === 'pending'" :error="error" @retry="refresh" />
      <div v-if="data?.items.length" class="stack">
        <NuxtLink v-for="ticket in data.items" :key="ticket.id" class="panel" :to="'/user/tickets/' + ticket.id">
          <span class="badge">{{ statusLabel[ticket.status] || ticket.status }}</span>
          <h2 style="font-size:18px">{{ ticket.product_title }}</h2>
          <p class="muted">{{ dateTime(ticket.created_at) }}</p>
        </NuxtLink>
      </div>
      <div v-else-if="data && !error" class="empty">还没有服务工单。</div>
    </ClientOnly>
  </div>
</template>

<script setup lang="ts">
import type {ProductSummary} from '@devstore/shared-types';import {money,statusLabel} from '@devstore/ui/format';import {ArrowUpRight,BookOpen} from '@lucide/vue';
defineProps<{product:ProductSummary}>();
</script>
<template><NuxtLink :to="'/products/'+product.slug" class="product-card">
 <div class="product-art" aria-hidden="true"><div><span class="badge purple">PROJECT BASED</span></div><div><div class="art-title">Build. Learn. Ship.</div><small>从一行代码，到一个完整项目</small></div></div>
 <div class="product-body"><div class="row"><span class="badge"><BookOpen :size="11"/> {{ statusLabel[product.type] || '商品' }}</span><span v-if="product.is_demo" class="badge amber">示例内容</span></div><h3>{{ product.title }}</h3><p>{{ product.description }}</p><div class="product-meta"><span class="price">{{ product.price_cents===0?'免费':money(product.price_cents) }}</span><span class="text-link row">{{ product.type==='tutorial'?'查看教程':'查看详情' }} <ArrowUpRight :size="14"/></span></div></div>
</NuxtLink></template>

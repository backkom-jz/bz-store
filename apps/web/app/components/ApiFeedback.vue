<script setup lang="ts">defineProps<{pending?:boolean;error?:unknown}>();defineEmits<{retry:[]}>()</script>
<template><p v-if="pending" class="muted" role="status" style="padding:30px 0">正在加载…</p><div v-else-if="error" class="error" role="alert"><p>{{ error instanceof Error?error.message:'加载失败，请稍后重试' }}</p><button class="btn secondary small" @click="$emit('retry')">重试</button></div></template>

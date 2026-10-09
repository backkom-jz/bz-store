import tailwindcss from '@tailwindcss/vite';
export default defineNuxtConfig({
 compatibilityDate:'2026-10-01',
 devtools:{enabled:false},
 css:['@devstore/ui/tokens.css','~/assets/main.css'],
 vite:{plugins:[tailwindcss()]},
 runtimeConfig:{apiInternalUrl:process.env.NUXT_API_INTERNAL_URL||'http://127.0.0.1:18080'},
 app:{head:{htmlAttrs:{lang:'zh-CN'},title:'DevStore · 技术，从学习到落地',meta:[{name:'description',content:'专注 Rust、Vue 与工程实践的技术教程。系统学习，动手构建。'},{name:'theme-color',content:'#0B1020'}]}},
 routeRules:{'/**':{headers:{'Cache-Control':'private, no-store'}}},
 experimental:{payloadExtraction:false},
 typescript:{strict:true}
});

<script setup lang="ts">
import {computed} from 'vue';
import MarkdownIt from 'markdown-it';
import DOMPurify from 'dompurify';
import hljs from 'highlight.js/lib/core';
import rust from 'highlight.js/lib/languages/rust';
import javascript from 'highlight.js/lib/languages/javascript';
import xml from 'highlight.js/lib/languages/xml';
import bash from 'highlight.js/lib/languages/bash';
hljs.registerLanguage('rust',rust);hljs.registerLanguage('javascript',javascript);hljs.registerLanguage('html',xml);hljs.registerLanguage('bash',bash);
const props=defineProps<{content:string}>();
const md=new MarkdownIt({html:false,linkify:false,typographer:false,highlight(code,lang){return lang&&hljs.getLanguage(lang)?hljs.highlight(code,{language:lang,ignoreIllegals:true}).value:'';}});
md.validateLink=(url:string)=>/^(https?:\/\/|mailto:|#|\/(?!\/))/i.test(url);
const html=computed(()=>{
 // Private reading is client-only. SSR outputs escaped text, never unsanitized HTML.
 if(typeof window==='undefined')return '';
 return DOMPurify.sanitize(md.render(props.content),{USE_PROFILES:{html:true},FORBID_TAGS:['style','form','input','iframe'],FORBID_ATTR:['style']});
});
</script>
<template><article class="markdown" v-html="html" /></template>

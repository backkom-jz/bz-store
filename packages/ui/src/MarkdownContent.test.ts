import {it,expect} from 'vitest';
import {mount} from '@vue/test-utils';
import MarkdownContent from './MarkdownContent.vue';
it('renders code and blocks raw HTML, events and executable URLs',()=>{
 const wrapper=mount(MarkdownContent,{props:{content:'# Lesson\n\n<script>alert(1)</script>\n<img src=x onerror=alert(1)>\n\n[unsafe](javascript:alert(1))\n\n```html\n<script>example()</script>\n```\n\n[docs](https://example.com)'}});
 expect(wrapper.find('h1').text()).toBe('Lesson');
 expect(wrapper.find('script').exists()).toBe(false);
 expect(wrapper.find('[onerror]').exists()).toBe(false);
 expect(wrapper.find('a[href^="javascript:"]').exists()).toBe(false);
 expect(wrapper.find('pre code').text()).toContain('<script>example()</script>');
 expect(wrapper.find('a[href="https://example.com"]').exists()).toBe(true);
});

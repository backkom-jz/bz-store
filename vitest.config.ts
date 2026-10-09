import {defineConfig} from 'vitest/config';
import vue from '@vitejs/plugin-vue';
export default defineConfig({plugins:[vue()],test:{environment:'jsdom',include:['packages/**/*.test.ts','apps/web/**/*.test.ts','apps/admin/**/*.test.ts'],restoreMocks:true}});

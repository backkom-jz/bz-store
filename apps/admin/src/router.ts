import { createRouter, createWebHistory } from 'vue-router';

declare module 'vue-router' {
  interface RouteMeta {
    public?: boolean;
  }
}
import { loadSession, session } from './session';

const router = createRouter({
  history: createWebHistory('/admin/'),
  routes: [
    { path: '/login', name: 'login', component: () => import('./views/Login.vue'), meta: { public: true } },
    { path: '/', redirect: '/payments' },
    { path: '/products', name: 'products', component: () => import('./views/Products.vue') },
    { path: '/products/new', name: 'product-new', component: () => import('./views/ProductEditor.vue') },
    { path: '/products/:id', name: 'product', component: () => import('./views/ProductEditor.vue') },
    { path: '/tutorials', name: 'tutorials', component: () => import('./views/Tutorials.vue') },
    { path: '/tutorials/new', name: 'tutorial-new', component: () => import('./views/TutorialEditor.vue') },
    { path: '/tutorials/:id', name: 'tutorial', component: () => import('./views/TutorialEditor.vue') },
    { path: '/payments', name: 'payments', component: () => import('./views/Payments.vue') },
    { path: '/orders', name: 'orders', component: () => import('./views/Orders.vue') },
    { path: '/orders/:id', name: 'order', component: () => import('./views/OrderDetail.vue') },
    { path: '/tickets', name: 'tickets', component: () => import('./views/Tickets.vue') },
    { path: '/tickets/:id', name: 'ticket', component: () => import('./views/TicketDetail.vue') },
    { path: '/blog', name: 'blog', component: () => import('./views/Blog.vue') },
    { path: '/blog/new', name: 'blog-new', component: () => import('./views/BlogEditor.vue') },
    { path: '/blog/:id', name: 'blog-post', component: () => import('./views/BlogEditor.vue') },
    { path: '/audit', name: 'audit', component: () => import('./views/AuditLogs.vue') },
  ],
});

router.beforeEach(async (to) => {
  if (!session.ready) await loadSession();
  if (to.meta.public) return true;
  if (!session.user?.is_admin) return { name: 'login', query: { returnTo: to.fullPath } };
});

export default router;

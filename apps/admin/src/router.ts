import { createRouter, createWebHistory } from 'vue-router';

declare module 'vue-router' {
  interface RouteMeta {
    public?: boolean;
    title?: string;
  }
}
import { loadSession, session } from './session';

const router = createRouter({
  history: createWebHistory('/admin/'),
  routes: [
    { path: '/login', name: 'login', component: () => import('./views/Login.vue'), meta: { public: true } },
    { path: '/', redirect: '/workbench' },
    { path: '/workbench', name: 'workbench', component: () => import('./views/Workbench.vue'), meta: { title: '运营工作台' } },
    { path: '/products', name: 'products', component: () => import('./views/Products.vue'), meta: { title: '商品管理' } },
    { path: '/products/new', name: 'product-new', component: () => import('./views/ProductEditor.vue'), meta: { title: '新建商品' } },
    { path: '/products/:id', name: 'product', component: () => import('./views/ProductEditor.vue'), meta: { title: '编辑商品' } },
    { path: '/tutorials', name: 'tutorials', component: () => import('./views/Tutorials.vue'), meta: { title: '教程内容' } },
    { path: '/tutorials/new', name: 'tutorial-new', component: () => import('./views/TutorialEditor.vue'), meta: { title: '新建教程' } },
    { path: '/tutorials/:id', name: 'tutorial', component: () => import('./views/TutorialEditor.vue'), meta: { title: '编辑教程' } },
    { path: '/payments', name: 'payments', component: () => import('./views/Payments.vue'), meta: { title: '付款审核' } },
    { path: '/orders', name: 'orders', component: () => import('./views/Orders.vue'), meta: { title: '订单管理' } },
    { path: '/orders/:id', name: 'order', component: () => import('./views/OrderDetail.vue'), meta: { title: '订单详情' } },
    { path: '/tickets', name: 'tickets', component: () => import('./views/Tickets.vue'), meta: { title: '服务工单' } },
    { path: '/tickets/:id', name: 'ticket', component: () => import('./views/TicketDetail.vue'), meta: { title: '工单详情' } },
    { path: '/blog', name: 'blog', component: () => import('./views/Blog.vue'), meta: { title: '博客文章' } },
    { path: '/blog/new', name: 'blog-new', component: () => import('./views/BlogEditor.vue'), meta: { title: '撰写文章' } },
    { path: '/blog/:id', name: 'blog-post', component: () => import('./views/BlogEditor.vue'), meta: { title: '编辑文章' } },
    { path: '/audit', name: 'audit', component: () => import('./views/AuditLogs.vue'), meta: { title: '操作记录' } },
  ],
});

router.beforeEach(async (to) => {
  if (!session.ready) await loadSession();
  if (to.meta.public) return true;
  if (!session.user?.is_admin) return { name: 'login', query: { returnTo: to.fullPath } };
});

export default router;

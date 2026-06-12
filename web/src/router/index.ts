import { createRouter, createWebHashHistory } from 'vue-router'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'chat', component: () => import('../views/ChatView.vue') },
    { path: '/settings', name: 'settings', component: () => import('../views/SettingsView.vue') },
    { path: '/live2d', name: 'live2d', component: () => import('../views/Live2DSettings.vue') },
    { path: '/avatar', name: 'avatar', component: () => import('../views/AvatarView.vue') },
  ],
})

export default router

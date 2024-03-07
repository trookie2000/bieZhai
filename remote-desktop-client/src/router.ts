import { createRouter, createWebHashHistory} from 'vue-router';
import screenOne from './components/screenOne.vue';
import RemotePanel from './components/RemotePanel.vue';

const routes = [
  {
    path: '/',
    component: RemotePanel,
  },
  {
    path: '/screenOne',
    component: screenOne, 
  },
  // 其他路由规则
];

const router = createRouter({
  history: createWebHashHistory(),
  routes,
});

export default router;

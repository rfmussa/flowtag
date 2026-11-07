import { createRouter, createWebHashHistory } from 'vue-router';

import QuickTag from '../views/QuickTag.vue';

// FlowTag is the app: every path lands on the tagging surface.
// The /quicktag path is kept (not '/') so route-conditional logic and
// saved hashes from older builds keep working.
const routes = [
    {
        path: '/quicktag',
        component: QuickTag
    },
    {
        path: '/:pathMatch(.*)*',
        redirect: '/quicktag'
    }
];

const router = createRouter({
    history: createWebHashHistory(),
    routes
});

export default router;

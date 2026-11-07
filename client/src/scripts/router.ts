import { createRouter, createWebHashHistory } from 'vue-router';

import QuickTag from '../views/QuickTag.vue';
import MiniPlayer from '../views/MiniPlayer.vue';

// FlowTag is the app: every path lands on the tagging surface.
// The /quicktag path is kept (not '/') so route-conditional logic and
// saved hashes from older builds keep working.
const routes = [
    {
        path: '/quicktag',
        component: QuickTag
    },
    {
        // Floating miniplayer window (opened by the shell, frameless)
        path: '/mini',
        component: MiniPlayer
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

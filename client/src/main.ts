import { createApp } from 'vue';
import { get1t } from './scripts/onetagger';
import router from './scripts/router';

// Style
import './style/app.scss';
import './style/tailwind.css';
import 'vue-sonner/style.css';

import App from './App.vue';


// Handle WebView events
// @ts-ignore
window.onWebviewEvent = (e) => {
    get1t().onOSMessage(e);
}


const app = createApp(App);

// Vue swallows component errors before window.onerror sees them:
// route them into the flight recorder too
app.config.errorHandler = (err, _instance, info) => {
    import('./scripts/diag').then((d) => d.diagError('vueError', {
        message: String(err).slice(0, 400),
        info,
        stack: (err as Error | undefined)?.stack?.slice(0, 1500),
    }));
    // eslint-disable-next-line no-console
    console.error(err);
};

app.use(router).mount('#app');

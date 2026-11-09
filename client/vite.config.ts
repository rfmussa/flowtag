import path from 'path';
import { fileURLToPath, URL } from 'node:url';
import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';
import { quasar, transformAssetUrls } from '@quasar/vite-plugin';
import vue from '@vitejs/plugin-vue';

/**
 * Quasar's stylesheet goes into a cascade layer below Tailwind's, so Tailwind
 * utilities win where both set a property. App styles stay unlayered and keep
 * beating both. Quasar's own `.flex` (display:flex + flex-wrap:wrap) is
 * dropped: the class name now belongs to Tailwind, whose `flex` must not wrap.
 * Removed together with Quasar.
 */
const QUASAR_FLEX_CLASS = /\.flex(?![\w-])/;

function layerQuasarCss() {
    return {
        postcssPlugin: 'flowtag-layer-quasar',
        // postcss is Vite's own dependency; typed structurally to avoid a direct one
        Once(root: any, { AtRule: AtRuleCtor, result }: any) {
            const from = (result.opts.from ?? '').replace(/\\/g, '/');
            if (!from.includes('/quasar/src/css/')) return;
            root.walkRules((rule: any) => {
                const kept = rule.selectors.filter((sel: string) => !QUASAR_FLEX_CLASS.test(sel));
                if (kept.length === 0) rule.remove();
                else if (kept.length !== rule.selectors.length) rule.selectors = kept;
            });
            const layer = new AtRuleCtor({ name: 'layer', params: 'quasar' });
            layer.append(root.nodes.slice());
            root.append(layer);
        },
    };
}


// https://vitejs.dev/config/
export default defineConfig({
    plugins: [
        vue({
            template: { transformAssetUrls }
        }),
        quasar({
            sassVariables: path.resolve('src/style/quasar.scss')
        }),
        tailwindcss()
    ],
    css: {
        postcss: { plugins: [layerQuasarCss()] }
    },
    resolve: {
        alias: {
            '@': fileURLToPath(new URL('./src', import.meta.url))
        }
    },
    server: {
        port: 8080
    },
    build: {
        assetsInlineLimit: 16 * 1024
    }
});

<template>
<div class='flex flex-col gap-3'>
    <ol class='list-decimal pl-5 text-sm text-ft-text-2 leading-relaxed'>
        <li>
            Create an app in the
            <button type='button' class='inline-flex items-center gap-0.5 underline text-ft-text cursor-pointer' @click='$1t.url("https://developer.spotify.com/dashboard")'>
                Spotify Developer dashboard<ExternalLinkIcon class='size-3' />
            </button>
        </li>
        <li>Set its redirect URL to <span class='ft-mono text-ft-text select-text'>{{ redirectUrl }}</span></li>
        <li>Paste the app's Client ID and Client Secret, then sign in</li>
    </ol>
    <form class='flex items-center gap-2' @submit.prevent='authorize'>
        <Input v-model='$1t.spotify.value.clientId' placeholder='Client ID' aria-label='Spotify Client ID' spellcheck='false' class='flex-1' />
        <Input
            v-model='$1t.spotify.value.clientSecret'
            :type='$1t.info.value.os == "macos" ? "text" : "password"'
            placeholder='Client Secret'
            aria-label='Spotify Client Secret'
            spellcheck='false'
            class='flex-1'
        />
        <Button type='submit' variant='default' :disabled='!$1t.spotify.value.clientId || !$1t.spotify.value.clientSecret'>Sign in</Button>
    </form>
</div>
</template>

<script lang='ts' setup>
import { ExternalLinkIcon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { get1t } from '../scripts/onetagger.js';
import { spotifyUrl } from '../scripts/utils';

const $1t = get1t();
const redirectUrl = spotifyUrl();

function authorize() {
    $1t.send("spotifyAuthorize", {
        clientId: $1t.spotify.value.clientId,
        clientSecret: $1t.spotify.value.clientSecret
    });
    // Save (using AF for compatibility with older settings)
    $1t.settings.value.audioFeatures.spotifyClientId = $1t.spotify.value.clientId;
    $1t.settings.value.audioFeatures.spotifyClientSecret = $1t.spotify.value.clientSecret;
    $1t.saveSettings();
}
</script>

<script setup>
import { RouterView } from "vue-router";
import { initEncryptedTunnel } from 'layer8-interceptor-rs'
const BACKEND_URL = import.meta.env.VITE_BACKEND_URL
const PROXY_URL = import.meta.env.VITE_PROXY_URL

try {
  initEncryptedTunnel({
    providers: [BACKEND_URL],
    proxy: PROXY_URL,
    staticPaths: [
      "/media",
      "/camera",
    ],
    cacheAssetLimit: 5 // if we want to cache assets at a limit of 5 MB
  }, "dev");
} catch (err) {
  console.error(".initEncryptedTunnel error: ", err)
}
</script>

<template>
  <div>
    <RouterView />
  </div>
</template>

<style scoped></style>

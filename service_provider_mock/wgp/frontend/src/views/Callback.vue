<!-- This complete code (CallBack View) is a part of Layer8 Component -->
<script setup>
import { computed, ref } from "vue";
import { onMounted } from 'vue'
import { useRouter } from 'vue-router'
import {fetch} from 'layer8-interceptor-rs'

const router = useRouter()
const code = ref(new URLSearchParams(window.location.search).get("code"))
const token = ref(localStorage.getItem("token") || null)
const BACKEND_URL =  import.meta.env.VITE_BACKEND_URL


onMounted(() => {
    setTimeout(() => {
        fetch(BACKEND_URL + "/api/login/layer8/auth", {
            method: "POST",
            headers: {
                "Content-Type": "Application/Json"
            },
            body: JSON.stringify({
                callback_url: window.location.href,
            })
        })
            .then(res => res.json())
            .then(data => {
                localStorage.setItem("token", data.token)
                router.push({ name: 'stress-test' })
            })
            .catch(err => console.log(err))
    }, 1000);
})
</script>

<template>
    <div>
        <h1>Login with layer8...</h1>
    </div>
</template>

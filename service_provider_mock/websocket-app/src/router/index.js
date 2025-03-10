import { createRouter, createWebHistory } from "vue-router";
import ChatView from "@/components/ChatView.vue";

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: "/",
      name: "chatView",
      component: ChatView,
    }
  ],
});

export default router;

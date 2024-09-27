import { createRouter, createWebHistory } from "vue-router";
import Tester from "../views/Tester.vue";

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: "/", // "home"
      name: "home",
      component: Tester,
    },
  ],
});

export default router;

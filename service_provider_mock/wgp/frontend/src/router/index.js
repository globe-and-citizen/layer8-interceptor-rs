import { createRouter, createWebHistory } from "vue-router";
import Home from "../views/Home.vue";
import LoginRegister from "../views/LoginRegister.vue";
import StressTest from "../views/StressTest.vue";
import Callback from "../views/Callback.vue";

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: "/",
      name: "loginRegister",
      component: LoginRegister,
    },
    {
      path: "/stress-test",
      name: "stress-test",
      component: StressTest,
    },
    {
      path: "/oauth2/callback",
      name: "oauth2-callback",
      component: Callback,
    },
    {
      path: "/home",
      name: "home",
      component: Home,
    },
  ],
});

export default router;

<script setup>
// Imports
import { computed, onMounted, ref, } from "vue";
import Navbar from "../components/Navbar.vue";
import { useRouter } from "vue-router";
import { fetch, _static } from 'layer8-interceptor-rs'

// Variables
const BACKEND_URL = import.meta.env.VITE_BACKEND_URL
const router = useRouter();
const registerUsername = ref("");
const registerPassword = ref("");
const loginEmail = ref("");
const loginPassword = ref("");
const profileImage = ref(null);
const isRegister = ref(false);
const isLoggedIn = computed(() => SpToken.value !== null);
const isContinueAnonymously = ref(false);
const SpToken = ref(localStorage.getItem("SP_TOKEN") || null);
const user = ref(JSON.parse(localStorage.getItem("_user")) || null); // ref(localStorage.getItem("_user") || null ) //?
const isLoading = ref(false);

const userProfileSrc = computed(() => {
  let val = user?.value.profile_image;
  if (val) {
    _static(user.value.profile_image).then((url) => {
      const element = document.getElementById("userProfileSrc");
      element.src = url;
      console.log("User profile image URL: ", url);
    }).catch((err) => {
      console.log("Error in getting user profile image: ", err);
    });
  }

  return val
});

const registerUser = async () => {
  try {
    console.log("username is: ", registerUsername.value)
    console.log("picture is: ", profileImage.value)

    let resp = fetch(BACKEND_URL + "/api/register", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        email: registerUsername.value,
        password: registerPassword.value,
        profile_image: profileImage.value,
      }),
    });
    console.log("resp: ", resp)
    alert("Registration successful!");
    isRegister.value = false;
  } catch (error) {

    console.log("username is: ", registerUsername.value)
    console.log("picture is: ", profileImage.value)

    console.log(error);
    alert("Registration failed!");
  }
};

const loginUser = async () => {
  if (loginEmail.value == "" || loginPassword.value == "") {
    console.log("Login failed. Input needed");
    throw new Error("input needed");
  }

  try {
    const response = await fetch(BACKEND_URL + "/api/login", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        email: loginEmail.value,
        password: loginPassword.value,
      }),
    });

    const data = await response.json();
    if (response.status !== 200) {
      throw new Error(data.error);
    }
    SpToken.value = data.token;
    user.value = data.user;
    localStorage.setItem("SP_TOKEN", data.token);
    localStorage.setItem("_user", JSON.stringify(data.user));
    alert("Login successful!");
  } catch (error) {
    alert(error.message);
  }
};

const continueAnonymously = () => {
  isContinueAnonymously.value = true;
  alert("You are now logged in anonymously!");
  router.push({ name: "home" });
};

const logoutUser = () => {
  SpToken.value = null;
  localStorage.removeItem("SP_TOKEN");
  localStorage.removeItem("_user");
  localStorage.clear();
  isContinueAnonymously.value = false;
};

const userName = computed(() => {
  if (SpToken.value && SpToken.value.split(".").length > 1) {
    const payload = JSON.parse(atob(SpToken.value.split(".")[1]));
    return payload.email;
  }
  return "";
});

const loginWithLayer8Popup = async () => {
  const response = await fetch(BACKEND_URL + "/api/login/layer8/auth")
  const data = await response.json()
  // create opener window
  const popup = window.open(data.authURL, "Login with Layer8", "width=600,height=600");

  window.addEventListener("message", async (event) => {
    if (event.data.redr) {
      setTimeout(() => {
        fetch(BACKEND_URL + "/api/login/layer8/auth", {
          method: "POST",
          headers: {
            "Content-Type": "Application/Json"
          },
          body: JSON.stringify({
            callback_url: event.data.redr,
          })
        })
          .then(res => res.json())
          .then(data => {
            localStorage.setItem("L8_TOKEN", data.token)
            router.push({ name: 'home' })
            popup.close();
          })
          .catch(err => console.log(err))
      }, 1000);
    }
  });
}

const uploadProfilePicture = async (e) => {
  e.preventDefault();
  isLoading.value = true;
  const file = e.target.files[0];
  const formdata = new FormData();
  formdata.append("file", file);
  fetch(BACKEND_URL + "/api/profile/upload", {
    method: "POST",
    headers: {
      "Content-Type": "multipart/form-data",
    },
    body: formdata,
  })
    .then((res) => res.json())
    .then(async (data) => {
      profileImage.value = data.url;
      const url = await _static(data.url);
      const element = document.getElementById("im");
      element.src = url;
    })
    .catch((err) => console.log("image upload err: ", err))
    .finally(() => {
      isLoading.value = false;
    });
};

</script>

<template>
  <div class="h-screen bg-primary flex flex-col">
    <Navbar></Navbar>
    <div class="bg-primary-content w-full flex justify-center items-center p-4 flex-1">
      <!-- LOGIN AND REGISTRATION SCREENS -->
      <div class="card w-auto bg-base-100 shadow-xl p-8 max-w-xs h-min" v-if="!isLoggedIn">
        <!-- REGISTRATION -->
        <div v-if="isRegister" class="flex gap-3 flex-col">
          <h2 class="text-lg font-bold ">Register</h2>
          <input v-model="registerUsername" placeholder="Username"
            class="input input-bordered input-primary w-full max-w-xs" />
          <input v-model="registerPassword" type="password" placeholder="Password"
            class="input input-bordered input-primary w-full max-w-xs" />
          <hr />
          <h1 class="text-dark pb-4 font-bold">Upload Profile Picture</h1>
          <input type="file" @change="uploadProfilePicture" />
          <div v-if="profileImage">
            <img id="im" />
          </div>
          <hr />
          <button class="btn btn-primary max-w-xs" @click="registerUser" :disabled="isLoading">
            <div v-if="isLoading" class="loading"></div>Register
          </button>
          <a class="block" @click="isRegister = false">Already registered? Login</a>
        </div>

        <!-- LOGIN -->
        <div v-if="!isRegister" class="flex gap-3 flex-col">
          <h2 class="text-lg font-bold">Login</h2>
          <input v-model="loginEmail" placeholder="default user: tester"
            class="input input-bordered input-primary w-full max-w-xs" />
          <input v-model="loginPassword" type="password" placeholder="default pass: 1234"
            class="input input-bordered input-primary w-full max-w-xs" />
          <button class="btn btn-primary max-w-xs" @click="loginUser">Login</button>
          <a class="block" @click="isRegister = true">Don't have an account? Register</a>
        </div>
      </div>

      <!-- CHOOSE FULLY ANONYMOUS BROWSING OR LAYER8 BROWSING SELECTION BOX -->
      <div v-if="isLoggedIn" class="card w-auto bg-base-100 shadow-xl p-8 max-w-xs">
        <!-- At this point, the user CAN have profile served by the S.P., however, their info will not be sinked and their opinions discounted as such -->
        <h1 class="text-dark pb-4 font-bold">
          Welcome {{ user?.email }}!
        </h1>

        <div v-if="userProfileSrc">
          <img id="userProfileSrc" />
          <br />
          <hr />
          <br />
        </div>
        <div class="flex flex-col gap-4" v-if="!isContinueAnonymously">
          <button class="btn " @click="continueAnonymously">
            Login Anonymously
          </button>
          <button class="btn " @click="loginWithLayer8Popup">
            Login with Layer8
          </button>
          <button class="btn " @click="logoutUser">Logout</button>
        </div>
      </div>
    </div>
  </div>
</template>
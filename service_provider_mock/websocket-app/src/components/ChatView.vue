<template>
  <div>
    <h1>Real-Time Chat</h1>
    <input type="text" v-model="message" @keyup.enter="sendMessage">
    <ul>
      <li v-for="message in messages" :key="message.id">
        {{ message.text }}
      </li>
    </ul>
  </div>
</template>

<script>
import { L8WebSocket } from 'layer8-interceptor-rs';

export default {
  name: 'ChatView',
  data() {
    return {
      message: '',
      messages: [],
    };
  },
  async mounted() {
    try {
      this.socket = new L8WebSocket();
      await this.socket.init({  
        proxy: 'ws://localhost:9086',
        url: 'ws://localhost:5001'
      });

      console.log('ws client is ready to use')
    } catch (error) {
      console.error(error);
    }

    console.log("check execution")

    this.socket.onmessage = (event) => {
      this.messages.push({ text: event.data, id: Math.random() });
    };

    this.socket.onopen = () => {
      console.log('Connected to the WebSocket server');
    };

    this.socket.onclose = () => {
      console.log('Disconnected from the WebSocket server');
    };
  },
  methods: {
    sendMessage() {
      this.socket.send(this.message);
      this.message = '';
    },
  },
};
</script>
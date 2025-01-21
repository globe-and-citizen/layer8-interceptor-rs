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
export default {
  name: 'ChatView',
  data() {
    return {
      message: '',
      messages: [],
    };
  },
  mounted() {
    this.socket = new WebSocket('ws://localhost:9086');

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
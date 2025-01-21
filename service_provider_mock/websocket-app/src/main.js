import { createApp } from 'vue'
import App from './App.vue'

createApp(App).mount('#app')


// import WebSocket from 'ws';
// import Vue from 'vue';

// const wss = new WebSocket.Server({ port: 8080 });

// wss.on('connection', (ws) => {
//   console.log('Client connected');

//   ws.on('message', (message) => {
//     console.log(`Received message => ${message}`);
//     ws.send(`Server received your message => ${message}`);
//   });

//   ws.on('close', () => {
//     console.log('Client disconnected');
//   });
// });
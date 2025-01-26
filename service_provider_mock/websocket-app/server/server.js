const express = require('express');
const app = express();
const server = require('http').createServer(app);
const wss = require('ws');
const wsServer = new wss.Server({ server });

app.use(express.static('public'));

function broadcast(msg) {
    console.log(msg);
    wsServer.clients.forEach(function each(client) {
        client.send(msg.toString());
    });
};

wsServer.on('connection', (ws) => {
    console.log('Client connected');

    ws.on('message', (message) => {
        console.log(`Received message => ${message}`);
        broadcast(message);
    });

    ws.on('close', () => {
        console.log('Client disconnected');
    });
});

server.listen(9086, () => {
    console.log('Server listening on port 9086');
});
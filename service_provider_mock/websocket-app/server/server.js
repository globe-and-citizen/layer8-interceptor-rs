const express = require('express');
const app = express();
const server = require('http').createServer(app);
const wss = require('ws');
const wsServer = new wss.Server({ server });

require('dotenv').config()
const port = process.env.PORT

app.use(express.static('public'));

function broadcast(msg) {
    console.log('echoing back the received msg');
    wsServer.clients.forEach(function each(client) {
        client.send(msg, (err) => {
            if (err != null || err != undefined)
                console.error("Logged an error on broadcast: ", err)
        });
    });
};

wsServer.on('connection', (ws) => {
    console.log('Client connected');

    ws.on('message', (message) => {
        console.log(`Received message => ${message}`);
        broadcast(message)
        // broadcast("Hello, World!");
    });

    ws.on('close', () => {
        console.log('Client disconnected');
    });
});

server.listen(port, () => {
    console.log(`Server listening on port: ${port}`);
});


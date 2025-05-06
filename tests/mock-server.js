const express = require('express')
const app = express()
const port = process.env.MOCK_SERVER_PORT || 9999

// Set CORS policy to allow all origins
const cors = require('cors')
app.use(cors({
    origin: '*',
    methods: ['GET', 'POST', 'PUT', 'DELETE', 'OPTIONS'],
    allowedHeaders: ['Content-Type', 'Authorization']
}))

app.use(express.urlencoded());


const test_cache = {};

app.get('/health_check', (req, res) => {
    const { query: { backend_url, client_id } } = req

    let return_ok = false;
    if (client_id) {
        if (!test_cache[client_id]) {
            test_cache[client_id] = 1
        } else {
            test_cache[client_id] = test_cache[client_id] + 1

            // if we are on our third request, let's set ok as true
            if (test_cache[client_id] === 3) {
                return_ok = true
            }
        }
    }

    switch (backend_url) {
        case "ok":
            console.log('Serving ok')
            return res.status(200).send('ok')

        case "service_unavailable":
            if (return_ok) {
                console.log('Serving ok after serving service unavailable 2 times')
                return res.status(200).send('ok')
            }

            console.log('Serving service unavailable')
            return res.status(503).send('Service Unavailable')

        case "too_many_requests":
            if (return_ok) {
                console.log('Serving ok after serving too many requests 2 times')
                return res.status(200).send('ok')
            }

            console.log('Serving too many requests')
            return res.status(429).send('Too Many Requests')

        case "internal_server_error":
            if (return_ok) {
                console.log('Serving ok after serving internal server error 2 times')
                return res.status(200).send('ok')
            }

            console.log('Serving internal server error')
            return res.status(500).send('Internal Server Error')

        default:
            console.log('Serving default ok')
            return res.status(200).send('ok')
    }
})

app.listen(port, () => {
    console.log(
        `\nThe mock backend is now listening on port ${port}.`
    )
})
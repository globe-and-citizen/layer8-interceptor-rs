# We've Got Poems

## Running the application

We assume you have followed the steps in the [main README](../../README.md) for the tools required to run this application.

Wwe additionally need to install [npm](https://www.npmjs.com/get-npm) to run the application.

1. At the root of the project, build the wasm module to create the `pkg` folder with the wasm module and the JavaScript bindings.

    ```sh
    wasm-pack build --target web
    ```

2. Start the server in the backend folder in a separate terminal

    ```sh
    cd backend
    node index.js
    ```

3. Get the frontend running and open the browser

    ```sh
    cd frontend
    npm install
    npm run build
    npm run preview
    ```

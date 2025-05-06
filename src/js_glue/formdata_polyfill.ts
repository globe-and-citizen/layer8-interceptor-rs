/* eslint-disable no-restricted-syntax */

/**
 * This is a paired down version of the `extractBody` function in `undici` that can convert a
 * `FormData` instance into a stream object that can be easily read out of.
 *
 * @license https://github.com/nodejs/undici/blob/e39a6324c4474c6614cac98b8668e3d036aa6b18/LICENSE
 * @see {@link https://github.com/nodejs/undici/blob/e39a6324c4474c6614cac98b8668e3d036aa6b18/lib/fetch/body.js#L31}
 */
async function extractBody(inputFormData: FormData, boundary: String) {
    const prefix = `--${boundary}\r\nContent-Disposition: form-data`;

    /*! formdata-polyfill. MIT License. Jimmy Wärting <https://jimmy.warting.se/opensource> */
    const escape = (str: string) => str.replace(/\n/g, '%0A').replace(/\r/g, '%0D').replace(/"/g, '%22');
    const normalizeLinefeeds = (value: string) => value.replace(/\r?\n|\r/g, '\r\n');

    const enc = new TextEncoder();
    const blobParts: (Uint8Array | string | Blob)[] = [];
    const rn = new Uint8Array([13, 10]); // '\r\n'
    let length: (number | null) = 0;
    let hasUnknownSizeValue = false;

    for (const [name, value] of inputFormData) {
        if (typeof value === 'string') {
            // console.log("String value: ", value);
            const chunk = enc.encode(
                `${prefix}; name="${escape(normalizeLinefeeds(name))}"\r\n\r\n${normalizeLinefeeds(value)}\r\n`,
            );
            blobParts.push(chunk);
            length += chunk.byteLength;
        } else {
            let file_contents = await value.arrayBuffer();

            const chunk = enc.encode(
                `${prefix}; name="${escape(normalizeLinefeeds(name))}"${value.name ? `; filename="${escape(value.name)}"` : ''
                }\r\nContent-Type: ${value.type || 'application/octet-stream;base64'}\r\n\r\n`,
            );

            blobParts.push(chunk, new Uint8Array(file_contents), rn);
            if (typeof value.size === 'number') {
                length += chunk.byteLength + value.size + rn.byteLength;
            } else {
                hasUnknownSizeValue = true;
            }
        }
    }

    const chunk = enc.encode(`--${boundary}--`);
    blobParts.push(chunk);
    length += chunk.byteLength;
    if (hasUnknownSizeValue) {
        length = null;
    }

    return blobParts
}

/**
 * Convert an instance of the `FormData` API into a Uint8Array.
 */
export async function parseFormDataToArray(form: FormData, boundary: String) {
    let body = await extractBody(form, boundary);
    let chunks = new Uint8Array();
    for (const part of body) {
        const temp = new Uint8Array(chunks.length + (part as Uint8Array).length);
        temp.set(chunks);
        temp.set(part as Uint8Array, chunks.length);
        chunks = temp;
    }

    // Sample output:
    //
    //     --AaB03x
    //     content-disposition: form-data; name="field1"
    //     content-type: text/plain;charset=windows-1250
    //     content-transfer-encoding: quoted-printable
    //
    //     Joe owes =80100.
    //     --AaB03x
    return chunks;
}
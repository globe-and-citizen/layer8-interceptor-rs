"use strict";
/* eslint-disable no-restricted-syntax */
var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
var __generator = (this && this.__generator) || function (thisArg, body) {
    var _ = { label: 0, sent: function() { if (t[0] & 1) throw t[1]; return t[1]; }, trys: [], ops: [] }, f, y, t, g = Object.create((typeof Iterator === "function" ? Iterator : Object).prototype);
    return g.next = verb(0), g["throw"] = verb(1), g["return"] = verb(2), typeof Symbol === "function" && (g[Symbol.iterator] = function() { return this; }), g;
    function verb(n) { return function (v) { return step([n, v]); }; }
    function step(op) {
        if (f) throw new TypeError("Generator is already executing.");
        while (g && (g = 0, op[0] && (_ = 0)), _) try {
            if (f = 1, y && (t = op[0] & 2 ? y["return"] : op[0] ? y["throw"] || ((t = y["return"]) && t.call(y), 0) : y.next) && !(t = t.call(y, op[1])).done) return t;
            if (y = 0, t) op = [op[0] & 2, t.value];
            switch (op[0]) {
                case 0: case 1: t = op; break;
                case 4: _.label++; return { value: op[1], done: false };
                case 5: _.label++; y = op[1]; op = [0]; continue;
                case 7: op = _.ops.pop(); _.trys.pop(); continue;
                default:
                    if (!(t = _.trys, t = t.length > 0 && t[t.length - 1]) && (op[0] === 6 || op[0] === 2)) { _ = 0; continue; }
                    if (op[0] === 3 && (!t || (op[1] > t[0] && op[1] < t[3]))) { _.label = op[1]; break; }
                    if (op[0] === 6 && _.label < t[1]) { _.label = t[1]; t = op; break; }
                    if (t && _.label < t[2]) { _.label = t[2]; _.ops.push(op); break; }
                    if (t[2]) _.ops.pop();
                    _.trys.pop(); continue;
            }
            op = body.call(thisArg, _);
        } catch (e) { op = [6, e]; y = 0; } finally { f = t = 0; }
        if (op[0] & 5) throw op[1]; return { value: op[0] ? op[1] : void 0, done: true };
    }
};

/**
 * This is a paired down version of the `extractBody` function in `undici` that can convert a
 * `FormData` instance into a stream object that can be easily read out of.
 *
 * @license https://github.com/nodejs/undici/blob/e39a6324c4474c6614cac98b8668e3d036aa6b18/LICENSE
 * @see {@link https://github.com/nodejs/undici/blob/e39a6324c4474c6614cac98b8668e3d036aa6b18/lib/fetch/body.js#L31}
 */
function extractBody(inputFormData, boundary) {
    return __awaiter(this, void 0, void 0, function () {
        var prefix, escape, normalizeLinefeeds, enc, blobParts, rn, length, hasUnknownSizeValue, _i, inputFormData_1, _a, name_1, value, chunk_1, file_contents, chunk_2, chunk;
        return __generator(this, function (_b) {
            switch (_b.label) {
                case 0:
                    prefix = "--".concat(boundary, "\r\nContent-Disposition: form-data");
                    escape = function (str) { return str.replace(/\n/g, '%0A').replace(/\r/g, '%0D').replace(/"/g, '%22'); };
                    normalizeLinefeeds = function (value) { return value.replace(/\r?\n|\r/g, '\r\n'); };
                    enc = new TextEncoder();
                    blobParts = [];
                    rn = new Uint8Array([13, 10]);
                    length = 0;
                    hasUnknownSizeValue = false;
                    _i = 0, inputFormData_1 = inputFormData;
                    _b.label = 1;
                case 1:
                    if (!(_i < inputFormData_1.length)) return [3 /*break*/, 5];
                    _a = inputFormData_1[_i], name_1 = _a[0], value = _a[1];
                    if (!(typeof value === 'string')) return [3 /*break*/, 2];
                    chunk_1 = enc.encode("".concat(prefix, "; name=\"").concat(escape(normalizeLinefeeds(name_1)), "\"\r\n\r\n").concat(normalizeLinefeeds(value), "\r\n"));
                    blobParts.push(chunk_1);
                    length += chunk_1.byteLength;
                    return [3 /*break*/, 4];
                case 2: return [4 /*yield*/, value.arrayBuffer()];
                case 3:
                    file_contents = _b.sent();
                    chunk_2 = enc.encode("".concat(prefix, "; name=\"").concat(escape(normalizeLinefeeds(name_1)), "\"").concat(value.name ? "; filename=\"".concat(escape(value.name), "\"") : '', "\r\nContent-Type: ").concat(value.type || 'application/octet-stream;base64', "\r\n\r\n"));
                    blobParts.push(chunk_2, new Uint8Array(file_contents), rn);
                    if (typeof value.size === 'number') {
                        length += chunk_2.byteLength + value.size + rn.byteLength;
                    }
                    else {
                        hasUnknownSizeValue = true;
                    }
                    _b.label = 4;
                case 4:
                    _i++;
                    return [3 /*break*/, 1];
                case 5:
                    chunk = enc.encode("--".concat(boundary, "--"));
                    blobParts.push(chunk);
                    length += chunk.byteLength;
                    if (hasUnknownSizeValue) {
                        length = null;
                    }
                    return [2 /*return*/, blobParts];
            }
        });
    });
}
/**
 * Convert an instance of the `FormData` API into a Uint8Array.
 */
export function parseFormDataToArray(form, boundary) {
    return __awaiter(this, void 0, void 0, function () {
        var body, chunks, _i, body_1, part, temp;
        return __generator(this, function (_a) {
            switch (_a.label) {
                case 0: return [4 /*yield*/, extractBody(form, boundary)];
                case 1:
                    body = _a.sent();
                    chunks = new Uint8Array();
                    for (_i = 0, body_1 = body; _i < body_1.length; _i++) {
                        part = body_1[_i];
                        temp = new Uint8Array(chunks.length + part.length);
                        temp.set(chunks);
                        temp.set(part, chunks.length);
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
                    return [2 /*return*/, chunks];
            }
        });
    });
}

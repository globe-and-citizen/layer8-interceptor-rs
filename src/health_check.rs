// use reqwest::header::HeaderValue;
// use serde::Deserialize;
// use url::Url;
// use wasm_bindgen::{JsError, UnwrapThrowExt};

// use crate::js_imports_prelude::*;

// // This the default retry offset in seconds.
// static RETRY_OFFSET: u64 = 2;

// #[derive(Debug, Clone, Deserialize)]
// struct HealthCheckResponse {
//     forward_proxy: Option<Message>,
//     reverse_proxy: Option<Message>,
// }

// #[derive(Debug, Clone, Deserialize)]
// struct Message {
//     status: u16,
//     message: Option<String>,
//     body_dump: Option<String>,
// }

// /// Represents all the transport error classifications that we will consider while doing a healthcheck.
// pub enum TransportError {
//     None,

//     /// This error indactes the network is not able to handle the request at the moment.
//     /// Transport errors include: request timeout, too many requests
//     Transient {
//         retry_after: Option<u64>,
//     },

//     /// This error indicates the network is not able to handle the request at all.
//     /// Transport errors include: network unreachable, connection refused
//     Fatal,
// }

// impl TransportError {
//     // This is a loose-mapping of HTTP status codes to transport errors, we mostly care about fatal errors
//     // and transient errors, so we ignore the rest.
//     fn new(status: u16, headers: Option<&reqwest::header::HeaderMap<HeaderValue>>) -> Self {
//         match status {
//             x if x == reqwest::StatusCode::SERVICE_UNAVAILABLE
//                 || x == reqwest::StatusCode::TOO_MANY_REQUESTS
//                 || x == reqwest::StatusCode::REQUEST_TIMEOUT =>
//             {
//                 // 503 Service Unavailable sometimes provides a retry-After header, which indicates how long the client should wait before retrying.
//                 let retry_after = match headers {
//                     Some(v) => v.get("Retry-After").and_then(|h| h.to_str().ok()).and_then(|h| h.parse::<u64>().ok()),
//                     None => None,
//                 };

//                 TransportError::Transient { retry_after }
//             }

//             // all other 5xx errors are considered fatal
//             x if x >= 500 => TransportError::Fatal,

//             // if error is not a 5xx error, we consider it a non-error
//             _ => TransportError::None,
//         }
//     }
// }

// fn process_health_message(msg: &Message, proxy_dbg: &str) -> std::result::Result<bool, JsError> {
//     let error = TransportError::new(msg.status, None);
//     match error {
//         TransportError::Transient { .. } => {
//             console_log!(&format!("The {} proxy is not healthy, but we can retry", proxy_dbg));
//             Ok(true)
//         }

//         TransportError::Fatal => {
//             console_log!(&format!("The {} proxy is not healthy, and not retryable...", proxy_dbg));
//             Err(JsError::new(&format!(
//                 "The {} proxy is not healthy, status code: {}, message: {:?}\n {:?}",
//                 proxy_dbg, msg.status, msg.message, msg.body_dump
//             )))
//         }

//         TransportError::None => {
//             console_log!(&format!("The {} proxy is healthy", proxy_dbg));
//             Ok(false)
//         }
//     }
// }

// // Only call this when there was a 200 response from the forward proxy, resposible for interprating the health checks from both the
// // forward proxy and reverse proxy.
// fn process_health_message_pair(health_check_response: HealthCheckResponse) -> Result<bool, JsError> {
//     match (health_check_response.forward_proxy, health_check_response.reverse_proxy) {
//         (Some(forward_proxy), Some(reverse_proxy)) => {
//             let forward_proxy_error = TransportError::new(forward_proxy.status, None);
//             let reverse_proxy_error = TransportError::new(forward_proxy.status, None);

//             match (forward_proxy_error, reverse_proxy_error) {
//                 (TransportError::Transient { .. }, TransportError::Transient { .. }) => {
//                     console_log!("Both the forward and reverse proxy are not healthy, but we can retry");
//                     return Ok(true);
//                 }

//                 (TransportError::Fatal, TransportError::Fatal) => {
//                     console_log!("The forward proxy and reverse proxy is healthy, and not retryable");
//                     return Err(JsError::new(&format!(
//                         "Both the forward and reverse proxy are not healthy, status codes: {} {}; respectively",
//                         forward_proxy.status, reverse_proxy.status
//                     )));
//                 }

//                 (_, TransportError::Fatal) => {
//                     console_log!("The reverse proxy is not healthy, and not retryable");
//                     return Err(JsError::new(&format!(
//                         "The reverse proxy is not healthy, status code: {}, message: {:?}\n {:?}",
//                         reverse_proxy.status, reverse_proxy.message, reverse_proxy.body_dump
//                     )));
//                 }

//                 (TransportError::Fatal, _) => {
//                     console_log!("The forward proxy is not healthy, and not retryable");
//                     return Err(JsError::new(&format!(
//                         "The forward proxy is not healthy, status code: {}, message: {:?}\n {:?}",
//                         forward_proxy.status, forward_proxy.message, forward_proxy.body_dump
//                     )));
//                 }

//                 _ => {}
//             }

//             if process_health_message(&forward_proxy, "forward")? || process_health_message(&reverse_proxy, "reverse")? {
//                 return Ok(true);
//             }

//             console_log!(&format!(
//                 "The reverse proxy and forward proxy are healthy, status codes: {} {}; respectively",
//                 reverse_proxy.status, forward_proxy.status
//             ));
//         }

//         (Some(forward_proxy), None) => {
//             let val = process_health_message(&forward_proxy, "forward")?;
//             return Ok(val);
//         }

//         (None, Some(reverse_proxy)) => {
//             let val = process_health_message(&reverse_proxy, "reverse")?;
//             return Ok(val);
//         }

//         (None, None) => return Err(JsError::new("The health check response is not valid")),
//     }

//     Ok(false)
// }

// /// This function helps the user avoid the need of reloading their page in the case the tunnel has not been established.
// pub async fn health_check(provider_url: &str, proxy_url: &str, client_id: Option<&str>) -> Result<(), JsError> {
//     let mut proxy_url = Url::parse(proxy_url).map_err(|e| JsError::new(&format!("Failed to parse proxy URL: {}", e)))?;
//     proxy_url.set_path("/health_check");
//     proxy_url.query_pairs_mut().clear().append_pair("backend_url", provider_url);
//     if let Some(client_id) = client_id {
//         proxy_url.query_pairs_mut().append_pair("client_id", client_id);
//     }

//     let mut retries = 0;
//     loop {
//         if retries == 3 {
//             // We have retried 3 times, we should give up
//             return Err(JsError::new("The tunnel is not open and cannot be opened after 3 retries"));
//         }

//         let resp = reqwest::Client::new()
//             .get(proxy_url.as_str())
//             .send()
//             .await
//             .map_err(|e| JsError::new(&format!("Failed to send request: {}", e)))?;

//         let transport_error = TransportError::new(resp.status().as_u16(), Some(resp.headers()));
//         if let TransportError::Transient { retry_after } = transport_error {
//             let retry = if let Some(retry_duration) = retry_after {
//                 // if the number of seconds requested is greater than 10, give up
//                 if retry_duration > 10 {
//                     return Err(JsError::new(&format!(
//                         "The tunnel is not open, upstream requested a retry after {} seconds",
//                         retry_duration
//                     )));
//                 }
//                 retry_duration
//             } else {
//                 RETRY_OFFSET ^ retries
//             };

//             retries += 1;

//             // Each time we calculate the exponential backoff by an exponent of {retry+1}
//             // e.g. 2, 4, 8
//             sleep(retry as i32).await?;
//             continue;
//         }

//         if let TransportError::Fatal = transport_error {
//             return Err(JsError::new("The tunnel is not open and cannot be opened"));
//         }

//         let body = resp
//             .bytes()
//             .await
//             .map_err(|e| JsError::new(&format!("Failed to read response body: {}", e)))?;

//         let health_check_response: HealthCheckResponse = serde_json::from_slice(&body).map_err(|e| {
//             console_log!(&format!("Failed to parse response body: {}", String::from_utf8_lossy(&body)));
//             JsError::new(&format!("Failed to parse response body: {}", e))
//         })?;

//         if process_health_message_pair(health_check_response)? {
//             retries += 1; // 0

//             // Each time we calculate the exponential backoff by an exponent of {retry+1}
//             // e.g. 2, 4, 8
//             sleep((RETRY_OFFSET ^ retries) as i32).await?;
//             continue;
//         }

//         break;
//     }

//     Ok(())
// }

// async fn sleep(delay_in_seconds: i32) -> Result<(), JsError> {
//     let delay_in_millis = delay_in_seconds * 1000;

//     let mut cb = |resolve: js_sys::Function, _: js_sys::Function| {
//         web_sys::window()
//             .expect_throw("Failed to get window object, this should be infallible")
//             .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay_in_millis)
//             .expect_throw("Failed to set timeout");
//     };

//     let p = js_sys::Promise::new(&mut cb);

//     wasm_bindgen_futures::JsFuture::from(p)
//         .await
//         .map_err(|e| JsError::new(&format!("Failed to set timeout: {:?}", e.as_string())))?;

//     Ok(())
// }

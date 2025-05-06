use url::Url;
use wasm_bindgen::{JsError, UnwrapThrowExt};

// This the default retry offset in seconds.
static RETRY_OFFSET: u64 = 2;

pub enum TransportError {
    None,

    /// This error indactes the network is not able to handle the request at the moment.
    /// Transport errors include: request timeout, too many requests
    Transient {
        retry_after: Option<u64>,
    },

    /// This error indicates the network is not able to handle the request at all.
    /// Transport errors include: network unreachable, connection refused
    Fatal,
}

// This is a loose-mapping of HTTP status codes to transport errors, we mostly care about fatal errors
// and transient errors, so we ignore the rest.
impl From<reqwest::Response> for TransportError {
    fn from(value: reqwest::Response) -> Self {
        match value.status() {
            reqwest::StatusCode::SERVICE_UNAVAILABLE => {
                // 503 Service Unavailable sometimes provides a retry-After header, which indicates how long the client should wait before retrying.
                let retry_after = value
                    .headers()
                    .get("Retry-After")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|h| h.parse::<u64>().ok());

                TransportError::Transient { retry_after }
            }

            // Other transient errors
            reqwest::StatusCode::TOO_MANY_REQUESTS | reqwest::StatusCode::REQUEST_TIMEOUT => TransportError::Transient { retry_after: None },

            // all other 5xx errors are considered fatal
            x if x.as_u16() >= 500 => TransportError::Fatal,

            // if error is not a 5xx error, we consider it a non-error
            _ => TransportError::None,
        }
    }
}

/// This function helps the user avoid the need of reloading their page in the case the tunnel has not been established.
pub async fn health_check(provider_url: &str, proxy_url: &str, client_id: Option<&str>) -> Result<(), JsError> {
    let mut proxy_url = Url::parse(proxy_url).map_err(|e| JsError::new(&format!("Failed to parse proxy URL: {}", e)))?;
    proxy_url.set_path("/health_check");
    proxy_url.query_pairs_mut().clear().append_pair("backend_url", provider_url);
    if let Some(client_id) = client_id {
        proxy_url.query_pairs_mut().append_pair("client_id", client_id);
    }

    let mut retries = 0;
    loop {
        if retries == 3 {
            // We have retried 3 times, we should give up
            return Err(JsError::new("The tunnel is not open and cannot be opened after 3 retries"));
        }

        let resp = reqwest::Client::new()
            .get(proxy_url.as_str())
            .send()
            .await
            .map_err(|e| JsError::new(&format!("Failed to send request: {}", e)))?;

        match TransportError::from(resp) {
            // The tunnel is not open, but we can retry after the specified time
            TransportError::Transient { retry_after } => {
                match retry_after {
                    Some(retry_after) => {
                        // if the number of seconds requested is greater than 10, give up
                        if retry_after > 10 {
                            return Err(JsError::new(&format!(
                                "The tunnel is not open, upstream requested a retry after {} seconds",
                                retry_after
                            )));
                        }

                        sleep(retry_after as i32).await?;
                    }
                    None => {
                        retries += 1; // 0

                        // Each time we calculate the exponential backoff by an exponent of {retry+1}
                        // e.g. 2, 4, 8
                        sleep((RETRY_OFFSET ^ retries) as i32).await?;
                    }
                }
            }

            // The tunnel is not open and we should give up
            TransportError::Fatal => {
                return Err(JsError::new("The tunnel is not open and cannot be opened"));
            }

            // The tunnel is open
            TransportError::None => break,
        }
    }

    Ok(())
}

async fn sleep(delay_in_seconds: i32) -> Result<(), JsError> {
    let delay_in_millis = delay_in_seconds * 1000;

    let mut cb = |resolve: js_sys::Function, _: js_sys::Function| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, delay_in_millis)
            .expect_throw("Failed to set timeout");
    };

    let p = js_sys::Promise::new(&mut cb);

    wasm_bindgen_futures::JsFuture::from(p)
        .await
        .map_err(|e| JsError::new(&format!("Failed to set timeout: {:?}", e.as_string())))?;

    Ok(())
}

#[cfg(test)]
mod tests {}

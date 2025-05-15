// use uuid::Uuid;
// use wasm_bindgen_test::*;

// use layer8_interceptor_rs::health_check::health_check;

// #[allow(dead_code)]
// static PORT: &str = "9999";

// wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

// #[allow(dead_code)]
// #[wasm_bindgen_test]
// async fn server_status_ok() {
//     if let Err(e) = health_check("ok", &format!("http://localhost:{}", PORT), None).await {
//         panic!("Failed to check health: {:?}", e);
//     }
// }

// #[allow(dead_code)]
// #[wasm_bindgen_test]
// async fn server_status_unavailable_failed() {
//     if let Ok(()) = health_check("service_unavailable", &format!("http://localhost:{}", PORT), None).await {
//         panic!("Expected health check to fail, but it succeeded");
//     }
// }

// #[allow(dead_code)]
// #[wasm_bindgen_test]
// async fn server_simulate_unavailable_pass_on_retry() {
//     let client_id = Uuid::new_v4().to_string();
//     if let Err(e) = health_check("service_unavailable", &format!("http://localhost:{}", PORT), Some(client_id.as_str())).await {
//         panic!("Expected health check to pass after 3 reties, but it failed, {:?}", e);
//     }
// }

// #[allow(dead_code)]
// #[wasm_bindgen_test]
// async fn server_too_many_requests_failed() {
//     if let Ok(()) = health_check("too_many_requests", &format!("http://localhost:{}", PORT), None).await {
//         panic!("Expected health check to fail, but it succeeded");
//     }
// }

// #[allow(dead_code)]
// #[wasm_bindgen_test]
// async fn server_too_many_requests_pass_on_retry() {
//     let client_id = Uuid::new_v4().to_string();
//     if let Err(e) = health_check("too_many_requests", &format!("http://localhost:{}", PORT), Some(client_id.as_str())).await {
//         panic!("Expected health check to pass after 3 reties, but it failed, {:?}", e);
//     }
// }

// #[allow(dead_code)]
// #[wasm_bindgen_test]
// async fn server_internal_server_error_failed() {
//     if let Ok(()) = health_check("internal_server_error", &format!("http://localhost:{}", PORT), None).await {
//         panic!("Expected health check to fail, but it succeeded");
//     }
// }

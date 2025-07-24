use std::time::{Duration, Instant};
use std::net::SocketAddr;
use std::error::Error;

use reqwest::StatusCode;
use tokio::task::JoinHandle;
use serde::{Deserialize, Serialize};
use axum::{
    Router,
    extract::{Path, Json},
    http::StatusCode as AxumStatusCode,
    response::Json as JsonResponse,
    routing::{get, post},
};
use tokio::time::sleep;
use rand::Rng;

// 服务器配置
#[derive(Debug, Clone, Deserialize, Serialize)]
struct ServerConfig {
    port: u16,
    name: String,
    version: String,
    failure_rate: f64,
    min_delay_ms: u32,
    max_delay_ms: u32,
}

// Echo 请求负载
#[derive(Debug, Clone, Deserialize, Serialize)]
struct EchoRequest {
    message: Option<String>,
    #[serde(default)]
    data: serde_json::Value,
}

// 健康检查端点
async fn health_check() -> JsonResponse<serde_json::Value> {
    JsonResponse(serde_json::json!({
        "status": "UP",
        "name": "test-backend",
        "version": "1.0.0",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// Echo 端点 - 返回请求体
async fn echo(Json(payload): Json<EchoRequest>) -> JsonResponse<serde_json::Value> {
    JsonResponse(serde_json::json!({
        "message": "Echo response",
        "data": payload,
        "server": "test-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// 延迟响应端点 - 延迟指定的时间
async fn delayed_response(Path(duration_ms): Path<u64>) -> JsonResponse<serde_json::Value> {
    // 模拟显式延迟
    sleep(Duration::from_millis(duration_ms)).await;
    
    JsonResponse(serde_json::json!({
        "message": format!("Delayed response ({}ms)", duration_ms),
        "server": "test-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// 错误响应端点 - 返回指定的状态码
async fn error_response(Path(status_code): Path<u16>) -> (AxumStatusCode, JsonResponse<serde_json::Value>) {
    // 从提供的值创建状态码，如果无效则默认为 500
    let status = AxumStatusCode::from_u16(status_code).unwrap_or(AxumStatusCode::INTERNAL_SERVER_ERROR);
    
    (status, JsonResponse(serde_json::json!({
        "error": format!("Error response with status {}", status_code),
        "server": "test-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })))
}

// 运行测试服务器
async fn run_test_server(config: ServerConfig) -> Result<(), Box<dyn Error>> {
    // 定义要绑定的地址
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    
    // 构建带有路由的应用程序
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/echo", post(echo))
        .route("/delay/:duration_ms", get(delayed_response))
        .route("/error/:status_code", get(error_response));
    
    // 启动服务器
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}

// 辅助函数来启动测试服务器
async fn start_test_server(port: u16) -> (JoinHandle<()>, ServerConfig) {
    let config = ServerConfig {
        port,
        name: "test-backend".to_string(),
        version: "1.0.0".to_string(),
        failure_rate: 0.0, // 测试中没有随机失败
        min_delay_ms: 0,   // 测试中没有随机延迟
        max_delay_ms: 0,
    };
    
    let config_clone = config.clone();
    let handle = tokio::spawn(async move {
        let _ = run_test_server(config_clone).await;
    });
    
    // 给服务器一点时间启动
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    (handle, config)
}

#[tokio::test]
async fn test_health_check() {
    let port = 3010;
    let (handle, _) = start_test_server(port).await;
    
    let client = reqwest::Client::new();
    let response = client
        .get(&format!("http://localhost:{}/health", port))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = response.json::<serde_json::Value>().await.expect("Failed to parse JSON");
    assert_eq!(body["status"], "UP");
    assert_eq!(body["name"], "test-backend");
    
    // 清理
    handle.abort();
}

#[tokio::test]
async fn test_echo_endpoint() {
    let port = 3011;
    let (handle, _) = start_test_server(port).await;
    
    let client = reqwest::Client::new();
    let test_message = "Hello, world!";
    
    let response = client
        .post(&format!("http://localhost:{}/echo", port))
        .json(&serde_json::json!({
            "message": test_message,
            "data": {
                "test": true,
                "value": 42
            }
        }))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status(), StatusCode::OK);
    
    let body = response.json::<serde_json::Value>().await.expect("Failed to parse JSON");
    assert_eq!(body["message"], "Echo response");
    assert_eq!(body["data"]["message"], test_message);
    assert_eq!(body["data"]["data"]["test"], true);
    assert_eq!(body["data"]["data"]["value"], 42);
    
    // 清理
    handle.abort();
}

#[tokio::test]
async fn test_delayed_response() {
    let port = 3012;
    let (handle, _) = start_test_server(port).await;
    
    let client = reqwest::Client::new();
    let delay_ms = 200;
    
    let start = Instant::now();
    let response = client
        .get(&format!("http://localhost:{}/delay/{}", port, delay_ms))
        .send()
        .await
        .expect("Failed to send request");
    let elapsed = start.elapsed();
    
    assert_eq!(response.status(), StatusCode::OK);
    assert!(elapsed.as_millis() >= delay_ms as u128, "Response was too fast");
    
    let body = response.json::<serde_json::Value>().await.expect("Failed to parse JSON");
    assert!(body["message"].as_str().unwrap().contains(&format!("{}ms", delay_ms)));
    
    // 清理
    handle.abort();
}

#[tokio::test]
async fn test_error_response() {
    let port = 3013;
    let (handle, _) = start_test_server(port).await;
    
    let client = reqwest::Client::new();
    let status_code = 418; // I'm a teapot
    
    let response = client
        .get(&format!("http://localhost:{}/error/{}", port, status_code))
        .send()
        .await
        .expect("Failed to send request");
    
    assert_eq!(response.status().as_u16(), status_code);
    
    let body = response.json::<serde_json::Value>().await.expect("Failed to parse JSON");
    assert!(body["error"].as_str().unwrap().contains(&format!("{}", status_code)));
    
    // 清理
    handle.abort();
}
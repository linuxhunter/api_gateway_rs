use std::net::SocketAddr;
use std::time::Duration;
use std::error::Error;

use axum::{
    Router,
    extract::{Path, Json},
    http::StatusCode,
    response::Json as JsonResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
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
        "name": "example-backend",
        "version": "1.0.0",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// Echo 端点 - 返回请求体
async fn echo(Json(payload): Json<EchoRequest>) -> JsonResponse<serde_json::Value> {
    // 模拟随机失败 (10% 的概率)
    if rand::thread_rng().gen::<f64>() < 0.1 {
        return JsonResponse(serde_json::json!({
            "error": "Random failure occurred",
            "server": "example-backend",
        }));
    }
    
    // 模拟随机延迟 (10-100ms)
    let delay_ms = rand::thread_rng().gen_range(10..=100);
    sleep(Duration::from_millis(delay_ms)).await;
    
    JsonResponse(serde_json::json!({
        "message": "Echo response",
        "data": payload,
        "server": "example-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// 延迟响应端点 - 延迟指定的时间
async fn delayed_response(Path(duration_ms): Path<u64>) -> JsonResponse<serde_json::Value> {
    // 模拟显式延迟
    sleep(Duration::from_millis(duration_ms)).await;
    
    JsonResponse(serde_json::json!({
        "message": format!("Delayed response ({}ms)", duration_ms),
        "server": "example-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// 错误响应端点 - 返回指定的状态码
async fn error_response(Path(status_code): Path<u16>) -> (StatusCode, JsonResponse<serde_json::Value>) {
    // 从提供的值创建状态码，如果无效则默认为 500
    let status = StatusCode::from_u16(status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    
    (status, JsonResponse(serde_json::json!({
        "error": format!("Error response with status {}", status_code),
        "server": "example-backend",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // 创建服务器配置
    let config = ServerConfig {
        port: 3000,
        name: "example-backend".to_string(),
        version: "1.0.0".to_string(),
        failure_rate: 0.1, // 10% 的失败率
        min_delay_ms: 10,  // 最小 10ms 延迟
        max_delay_ms: 100, // 最大 100ms 延迟
    };
    
    // 定义要绑定的地址
    let addr = SocketAddr::from(([127, 0, 0, 1], config.port));
    
    println!("Starting example backend server on port {}", config.port);
    println!("Server will simulate random failures with rate: {}", config.failure_rate);
    println!("Server will add random delays between {}ms and {}ms", config.min_delay_ms, config.max_delay_ms);
    
    // 构建带有路由的应用程序
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/echo", post(echo))
        .route("/delay/:duration_ms", get(delayed_response))
        .route("/error/:status_code", get(error_response));
    
    println!("Backend server listening on {}", addr);
    
    // 启动服务器
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await?;
    
    Ok(())
}
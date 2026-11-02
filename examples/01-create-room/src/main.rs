//! 联机大厅建房最小示例。
//!
//! 演示三件事：
//! 1. 不带 `Content-Type`（这是最容易踩的坑）
//! 2. 直连不走系统代理（与官方客户端一致）
//! 3. 从响应里提取 `room_id`

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use reqwest::Client;
use serde_json::{json, Value};

const BASE_URL: &str = "https://x19apigatewayobt.nie.netease.com";
const USER_AGENT: &str = "WPFLauncher/1.15.31.20647";

#[derive(Debug, Parser)]
#[command(about = "联机大厅建房最小示例")]
struct Cli {
    /// 数字账号 ID（请求头 user-id）
    #[arg(long)]
    user_id: String,
    /// 16 位会话 token（请求头 user-token）
    #[arg(long)]
    token: String,
    /// 房间名称
    #[arg(long)]
    room_name: String,
    /// 作品/地图 ID（地图未下载会失败）
    #[arg(long)]
    res_id: String,
    /// 人数上限
    #[arg(long, default_value_t = 2)]
    max_count: u32,
    /// 公开性，0 表示公开
    #[arg(long, default_value_t = 0)]
    visibility: i32,
    /// 房间密码，留空表示无密码
    #[arg(long, default_value = "")]
    password: String,
}

/// 生成 32 位字母数字的追踪 ID（与客户端格式一致）。
fn trace_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut state = seed as u64 | 1;
    (0..32)
        .map(|_| {
            // 简单的 xorshift，仅用于生成无校验意义的 trace id
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            CHARSET[(state % CHARSET.len() as u64) as usize] as char
        })
        .collect()
}

/// 统一 POST：故意不用 `.json()`，避免自动补 `Content-Type`。
async fn post(client: &Client, cli: &Cli, path: &str, body: &Value) -> Result<Value> {
    let payload = serde_json::to_vec(body)?;
    let resp = client
        .post(format!("{BASE_URL}{path}"))
        .header("X_TRACE_ID", trace_id())
        .header("user-id", &cli.user_id)
        .header("user-token", &cli.token)
        .header("User-Agent", USER_AGENT)
        .header("Accept-Encoding", "gzip, deflate")
        .body(payload)
        .send()
        .await
        .context("发送请求失败")?;

    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("HTTP {status}: {text}"));
    }
    serde_json::from_str(&text).with_context(|| format!("响应不是 JSON: {text}"))
}

/// 从响应里提取 room_id，兼容常见命名。
fn extract_room_id(entity: &Value) -> Option<String> {
    for key in ["room_id", "roomId", "roomID", "id"] {
        if let Some(v) = entity.get(key) {
            if let Some(s) = v.as_str() {
                return Some(s.to_string());
            }
            if v.is_number() {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .no_proxy()
        .build()?;

    let body = json!({
        "room_name": cli.room_name,
        "max_count": cli.max_count,
        "visibility": cli.visibility,
        "res_id": cli.res_id,
        "save_id": "",
        "password": cli.password,
    });

    println!("正在建房: room_name={}", cli.room_name);
    let resp = post(&client, &cli, "/online-lobby-room", &body).await?;

    let code = resp.get("code").and_then(|v| v.as_i64()).unwrap_or(-1);
    let message = resp.get("message").and_then(|v| v.as_str()).unwrap_or("");

    if code != 0 {
        eprintln!("建房失败: code={code} message={message}");
        eprintln!("常见原因:");
        eprintln!("  4  → body 未被解析（Content-Type 陷阱）");
        eprintln!("  10 → token 已失效");
        eprintln!("  22 → 账号在别处登录（客户端正在运行）");
        std::process::exit(1);
    }

    match resp.get("entity").and_then(extract_room_id) {
        Some(room_id) => println!("建房成功，room_id = {room_id}"),
        None => println!("建房成功，但响应中没有可识别的 room_id: {resp}"),
    }

    Ok(())
}

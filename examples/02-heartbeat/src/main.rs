//! 建房 + 心跳保活示例。
//!
//! 状态机：
//! ```text
//! 建房 → 每 30 秒心跳 → 连续失败 N 次 → 等待退避 → 重新建房
//! ```
//!
//! 两个容易忘的点都在代码里标出来了：
//! 1. 重建前必须等待，避免高频建房触发频控
//! 2. 重建后 room_id 会变

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

const BASE_URL: &str = "https://x19apigatewayobt.nie.netease.com";
const USER_AGENT: &str = "WPFLauncher/1.15.31.20647";

#[derive(Debug, Parser)]
#[command(about = "建房 + 心跳保活示例")]
struct Cli {
    #[arg(long)]
    user_id: String,
    #[arg(long)]
    token: String,
    #[arg(long)]
    room_name: String,
    #[arg(long)]
    res_id: String,
    #[arg(long, default_value_t = 2)]
    max_count: u32,
    /// 心跳间隔（秒），官方客户端实测为 30
    #[arg(long, default_value_t = 30)]
    interval_secs: u64,
    /// 连续失败多少次判定掉线
    #[arg(long, default_value_t = 3)]
    max_failures: u32,
    /// 重建前的等待时间（秒）
    #[arg(long, default_value_t = 10)]
    retry_delay_secs: u64,
}

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
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            CHARSET[(state % CHARSET.len() as u64) as usize] as char
        })
        .collect()
}

/// 统一 POST：用 `.body()` 而非 `.json()`，避免自动补 Content-Type。
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

fn api_code(resp: &Value) -> (i64, String) {
    (
        resp.get("code").and_then(|v| v.as_i64()).unwrap_or(-1),
        resp.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    )
}

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

/// 建一次房，成功返回 room_id。
async fn create_room(client: &Client, cli: &Cli) -> Result<String> {
    let body = json!({
        "room_name": cli.room_name,
        "max_count": cli.max_count,
        "visibility": 0,
        "res_id": cli.res_id,
        "save_id": "",
        "password": "",
    });

    let resp = post(client, cli, "/online-lobby-room", &body).await?;
    let (code, message) = api_code(&resp);
    if code != 0 {
        return Err(anyhow!("建房失败: code={code} message={message}"));
    }

    resp.get("entity")
        .and_then(extract_room_id)
        .ok_or_else(|| anyhow!("建房成功但响应中没有 room_id: {resp}"))
}

/// 发一次心跳。
async fn tick(client: &Client, cli: &Cli, room_id: &str) -> Result<()> {
    let body = json!({
        "room_id": room_id,
        "password": null,
        // 字段名就是 check_visibilily，官方拼写错误，不要修正
        "check_visibilily": false,
    });

    let resp = post(client, cli, "/online-lobby-member-tick", &body).await?;
    let (code, message) = api_code(&resp);
    if code != 0 {
        return Err(anyhow!("心跳失败: code={code} message={message}"));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .no_proxy()
        .build()?;

    let mut attempt: u32 = 0;

    loop {
        attempt += 1;
        println!("[建房] 第 {attempt} 次尝试");

        let room_id = match create_room(&client, &cli).await {
            Ok(id) => {
                println!("[建房] 成功，room_id = {id}");
                id
            }
            Err(e) => {
                eprintln!("[建房] 失败: {e}");
                tokio::time::sleep(Duration::from_secs(cli.retry_delay_secs)).await;
                continue;
            }
        };

        // 心跳阶段
        let mut failures = 0u32;
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("[退出] 收到 Ctrl-C");
                    return Ok(());
                }
                _ = tokio::time::sleep(Duration::from_secs(cli.interval_secs)) => {
                    match tick(&client, &cli, &room_id).await {
                        Ok(()) => {
                            failures = 0;
                            println!("[心跳] 成功 room_id={room_id}");
                        }
                        Err(e) => {
                            failures += 1;
                            eprintln!("[心跳] 失败（连续 {failures} 次）: {e}");
                            if failures >= cli.max_failures {
                                println!("[重建] 连续失败达阈值，准备重建");
                                break;
                            }
                        }
                    }
                }
            }
        }

        // 重建前必须等待，避免高频建房
        println!("[重建] 等待 {} 秒后重试", cli.retry_delay_secs);
        tokio::time::sleep(Duration::from_secs(cli.retry_delay_secs)).await;
    }
}

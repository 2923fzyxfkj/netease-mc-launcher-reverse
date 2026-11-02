//! `user-token` 有效性探针。
//!
//! 用途：判断一个 token 现在处于什么状态。响应码含义：
//!
//! | code | 含义 |
//! |---|---|
//! | 0  | 有效，可用 |
//! | 10 | 已失效（客户端会话已注销） |
//! | 22 | 有效但账号在别处登录（客户端正在运行） |
//! | 12 | token 格式或内容无效 |
//!
//! 探测接口用的是只读的 `/user-detail`，不会创建任何房间。

use anyhow::{Context, Result};
use clap::Parser;
use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

const BASE_URL: &str = "https://x19apigatewayobt.nie.netease.com";
const USER_AGENT: &str = "WPFLauncher/1.15.31.20647";

#[derive(Debug, Parser)]
#[command(about = "user-token 有效性探针")]
struct Cli {
    #[arg(long)]
    user_id: String,
    #[arg(long)]
    token: String,
    /// 连续探测次数
    #[arg(long, default_value_t = 1)]
    count: u32,
    /// 每次间隔（秒）
    #[arg(long, default_value_t = 5)]
    interval_secs: u64,
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

/// 用一个只读接口探测 token。
async fn probe(client: &Client, cli: &Cli, path: &str, body: &Value) -> Result<(i64, String)> {
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
        return Err(anyhow::anyhow!("HTTP {status}: {text}"));
    }

    let v: Value = serde_json::from_str(&text).with_context(|| format!("响应不是 JSON: {text}"))?;
    Ok((
        v.get("code").and_then(|x| x.as_i64()).unwrap_or(-1),
        v.get("message")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
    ))
}

fn explain(code: i64) -> &'static str {
    match code {
        0 => "有效，可直接使用",
        10 => "已失效（客户端会话已注销）",
        12 => "token 无效（格式或内容错误）",
        13 => "操作失败（可能频率过高）",
        22 => "有效，但账号在别处登录（客户端正在运行）",
        _ => "未知状态",
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let client = Client::builder()
        .timeout(Duration::from_secs(15))
        .no_proxy()
        .build()?;

    // /user-detail 是只读接口，Content-Length 为 0
    let body = Value::Null;

    for i in 1..=cli.count {
        println!("--- 第 {i}/{count} 次探测 ---", count = cli.count);
        match probe(&client, &cli, "/user-detail", &body).await {
            Ok((code, message)) => {
                println!("code={code}  message={message}");
                println!("判定: {}", explain(code));
                if code != 0 {
                    println!("提示: 若非 0，心跳与建房都会失败，需要重新获取凭证");
                }
            }
            Err(e) => eprintln!("探测失败: {e}"),
        }

        if i < cli.count {
            tokio::time::sleep(Duration::from_secs(cli.interval_secs)).await;
        }
    }

    Ok(())
}

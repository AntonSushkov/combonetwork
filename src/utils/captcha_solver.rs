use crate::constants::*;
use tokio::time::sleep;
use serde_json::{json, Value};
use std::time::{Duration};
use isahc::{AsyncReadResponseExt, HttpClient};
use log::{info, error};
use crate::utils::error::MyError;


pub async fn hcaptcha_task_proxyless(
    session: &HttpClient,
    website_url: &str,
    website_key: &str,
    cap_key: &str,
    ua: &str,
) -> Result<String, MyError> {

    let payload = json!({
        "clientKey": cap_key,
        "task":
        {
            "type":"HCaptchaTaskProxyless",
            "websiteURL":website_url,
            "websiteKey":website_key,
        }
    });

    let mut response = session.post_async("https://api.capmonster.cloud/createTask", payload.to_string())
        .await?;

    let text = response.text().await.unwrap_or_default();
    let response_data: Value = serde_json::from_str(&text).unwrap_or_default();
    // let response_data: Value = serde_json::from_str(&response.text_async().await?)?;
    // println!("|  | Response_data: {}", response_data);
    info!("| | Captcha - Solve...");

    if let Some(task_id) = response_data["taskId"].as_u64() {
        sleep(Duration::from_secs(5)).await;

        for _ in 0..MAX_RETRIES {
            let payload = json!({
                "clientKey":cap_key,
                "taskId": task_id,
                "userAgent": ua
            });

            let mut response = session.post_async("https://api.capmonster.cloud/getTaskResult/", payload.to_string())
                .await?;

            let text = response.text().await.unwrap_or_default();
            let task_result: Value = serde_json::from_str(&text).unwrap_or_default();
            // let task_result: Value = serde_json::from_str(&response.text_async().await?)?;
            if let Some(status) = task_result["status"].as_str() {
                if status == "ready" {
                    // println!("| | Status ready: {}", task_result);
                    if let Some(g_recaptcha_response) = task_result["solution"]["gRecaptchaResponse"].as_str() {
                        return Ok(g_recaptcha_response.to_string());

                    }
                } else if status == "processing" {
                    // println!("Status processing: {}", task_result);
                    sleep(Duration::from_secs(3)).await;
                    continue;
                } else {
                    error!("| | Captcha - Error");
                    break;
                }
            }
        }
    }

    Err(MyError::ErrorStr("Failed to solve the captcha.".to_string()))
}

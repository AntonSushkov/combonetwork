use std::collections::HashMap;
use http::Request;
use isahc::{AsyncReadResponseExt, HttpClient};
use serde_json::{json, Value};
use crate::utils::captcha_solver::{hcaptcha_task_proxyless};
use crate::utils::config::{Config};
use std::time::Duration;
use tokio::time::sleep;
use log::{info, error};

use crate::{
    constants::*,
};

async fn fingerprint(session: &HttpClient, ua: &str) -> Result<String, isahc::Error> {
    let url = "https://discord.com/api/v9/experiments";
    let request = Request::builder()
        .method("GET")
        .uri(url)
        .header("accept", "*/*")
        .header("accept-language", "en-US,en;q=0.5")
        .header("x-discord-locale", "en-US")
        .header("x-debug-options", "bugReporterEnabled")
        .header("user-agent", ua)
        .header("connection", "keep-alive")
        .header("sec-fetch-dest", "empty")
        .header("sec-fetch-mode", "cors")
        .header("sec-fetch-site", "same-origin")
        .header("te", "trailers")
        .header("x-super-properties", X_SUPER_PROPERTIES)
        .body(())?;
    let mut response = session.send_async(request).await?;

    let text = response.text().await.unwrap_or_default();
    let task_result: Value = serde_json::from_str(&text).unwrap_or_default();

    let mut fingerprints = String::new();
    if let Some(fingerprint) = task_result["fingerprint"].as_str() {
        fingerprints = fingerprint.to_string();
    }
    Ok(fingerprints)
}

pub async fn join_server(session: &HttpClient, token: &str, invite: &str, ua: &str, config: &Config,) -> Result<String, isahc::Error> {

    let url = format!("https://discord.com/api/v9/invites/{}", invite);
    let website_url = &url.clone();
    let request = Request::builder()
        .method("POST")
        .uri(&url)
        .header("authority", "discord.com")
        .header("authorization", token)
        .header("Content-Type", "application/json")
        .body("{}")?;
    let mut response = session.send_async(request).await?;

    if response.status() != reqwest::StatusCode::OK {
        let mut captcha_sitekey = String::new();
        let text = response.text().await.unwrap_or_default();
        let task_result: Value = serde_json::from_str(&text).unwrap_or_default();
        // println!("task_result: {:?}", task_result);
        if let Some(sitekey) = task_result["captcha_sitekey"].as_str() {
            captcha_sitekey = sitekey.to_string();
            // println!("captcha_sitekey: {}", captcha_sitekey);
        }
        let mut captcha_rqtoken = String::new();
        if let Some(rqtoken) = task_result["captcha_rqtoken"].as_str() {
            captcha_rqtoken = rqtoken.to_string();
            // println!("captcha_rqtoken: {}", captcha_rqtoken);
        }

        if let Some(captcha_key_array) = task_result["captcha_key"].as_array() {
            if let Some(captcha_key) = captcha_key_array.first().and_then(Value::as_str) {
                println!("Captcha : {}", captcha_key);
            }
        }


        for attempt in 1..= config.settings.max_retries_connect_server {
        info!("Connect Discord...");
        let cap_key = &config.settings.cap_key;
        let website_key = captcha_sitekey.as_str();
        // let website_key = "a9b5fb07-92ff-493f-86fe-352a2803b3df";
        let g_recaptcha_response = hcaptcha_task_proxyless(session, website_url, website_key, cap_key, ua).await.expect("Huinay s captchei");
        // println!("gRecaptchaResponse: {}", g_recaptcha_response);
        let x_fingerprint_res = fingerprint(&session, &ua).await;
        let x_fingerprint = match x_fingerprint_res {
            Ok(fp) => fp,
            Err(_) => "1153014433115816036.QVUfzwzt-SoWgusrFdol9nVeRfo".to_string(),
        };
        let mut data: HashMap<String, serde_json::Value> = HashMap::new();
        data.insert("captcha_key".to_string(), json!(g_recaptcha_response));

        let serialized_data = serde_json::to_string(&data).expect("Failed to serialize data");
        let referer = format!("https://discord.com/invite/{}", invite);
        let request1 = Request::builder()
            .method("POST")
            .uri(&url)
            .header("authority", "discord.com")
            .header("accept", "*/*")
            .header("accept-language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .header("authorization", token)
            .header("content-type", "application/json")
            .header("origin", "https://discord.com")
            .header("referer", referer)
            .header("sec-ch-ua", " ")
            .header("sec-ch-ua-mobile", "?0")
            .header("sec-ch-ua-platform", "\"Windows\"")
            .header("sec-fetch-dest", "empty")
            .header("sec-fetch-mode", "cors")
            .header("sec-fetch-site", "same-origin")
            .header("user-agent", ua)
            .header("X-Debug-Options", "sbugReporterEnabled")
            .header("X-Super-Properties", X_SUPER_PROPERTIES)
            .header("x-captcha-key", &captcha_rqtoken)
            .header("x-fingerprint", &x_fingerprint)
            .body(serialized_data)?;
        let mut response1 = session.send_async(request1).await?;
        // println!("response1: {:?}", response1);

            // let mut response1 = session.send_async(request1).await?;

            if response1.status() == reqwest::StatusCode::OK {
                return Ok("Successfully Join 0".to_string());
            } else {
                info!("Attempt {}: Couldn't connect to the channel in Discord, trying again.", attempt);
                // let text = response1.text().await.unwrap_or_default();
                // let task_result: Value = serde_json::from_str(&text).unwrap_or_default();
                // println!("response1_body: {:?}", task_result);
                sleep(Duration::from_secs(10)).await;

                // If this is the last attempt, return the error
                if attempt == config.settings.max_retries_connect_server {
                    return Ok("Failed Join after max attempts".to_string());
                }
            }
        }
        return Ok("Failed Join after max attempts".to_string());
    } else {
        return Ok("Successfully Join 1".to_string());
    }
}

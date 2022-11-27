use std::time::Duration;

use anyhow::{Ok, Result};
use reqwest::{header::HeaderMap, Client, Response, Url};
use serde::Deserialize;

pub struct HttpClient {
    client: Client,
    base_url: Url,
}

#[derive(Debug, Deserialize)]
pub struct DanmuInfo {
    pub data: DanmuInfoData,
}

#[derive(Debug, Deserialize)]
pub struct DanmuInfoData {
    pub token: String,
    pub host_list: Vec<WsHost>,
}

#[derive(Debug, Deserialize)]
pub struct WsHost {
    pub host: String,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base_url: Url::parse("https://api.live.bilibili.com")?,
        })
    }

    async fn get(
        &self,
        path: &str,
        query: Option<&[(&str, &str)]>,
        headers: Option<HeaderMap>,
    ) -> Result<Response> {
        let resp = self
            .client
            .get(self.base_url.join(path)?)
            .query(query.unwrap_or_default())
            .headers(headers.unwrap_or_default())
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?;

        Ok(resp)
    }

    pub async fn get_dammu_info(&self, room_id: u64) -> Result<DanmuInfo> {
        let resp = self
            .get(
                &format!("xlive/web-room/v1/index/getDanmuInfo?id={}&type=0", room_id),
                None,
                None,
            )
            .await?
            .json::<DanmuInfo>()
            .await?;

        Ok(resp)
    }
}

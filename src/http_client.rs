use pct_str::{PctString, URIReserved};
use regex::Regex;
use reqwest::{header::HeaderMap, Client, Response};
use serde::Deserialize;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use url::Url;

use crate::FelgensResult;

pub struct HttpClient {
    client: Client,
    base_url: Url,
    header: HeaderMap,
    wbi_key: RwLock<Option<String>>,
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

#[derive(Debug, Deserialize)]
pub struct RoomInit {
    data: RoomInitData,
}

#[derive(Debug, Deserialize)]
pub struct RoomInitData {
    room_id: u64,
}

impl HttpClient {
    pub fn new(cookies: &str) -> FelgensResult<Self> {
        let mut header = HeaderMap::new();
        header.insert("cookie", cookies.parse().unwrap());

        Ok(Self {
            client: Client::new(),
            base_url: Url::parse("https://api.live.bilibili.com")?,
            header,
            wbi_key: RwLock::new(None),
        })
    }

    async fn get(&self, path: &str, query: Option<&[(&str, &str)]>) -> FelgensResult<Response> {
        let resp = self
            .client
            .get(self.base_url.join(path)?)
            .query(query.unwrap_or_default())
            .headers(self.header.clone())
            .timeout(Duration::from_secs(30))
            .send()
            .await?
            .error_for_status()?;

        Ok(resp)
    }

    pub async fn get_dammu_info(&self, room_id: u64) -> FelgensResult<DanmuInfo> {
        let parameters = serde_json::json!({
            "id": room_id.to_string(),
            "type": "0",
            "web_location": "444.8"
        });
        let sign = self.get_sign(parameters).await?;
        let resp = self
            .get(
                &format!("xlive/web-room/v1/index/getDanmuInfo?{}", sign),
                None,
            )
            .await?
            .json::<DanmuInfo>()
            .await?;

        Ok(resp)
    }

    pub async fn get_room_id(&self, room_id: u64) -> FelgensResult<u64> {
        let resp = self
            .get(
                &format!("room/v1/Room/room_init?id={}?&from=room", room_id),
                None,
            )
            .await?
            .json::<RoomInit>()
            .await?
            .data
            .room_id;

        Ok(resp)
    }

    async fn get_wbi_key(&self) -> FelgensResult<String> {
        if self.wbi_key.read().await.is_some() {
            return Ok(self.wbi_key.read().await.clone().unwrap());
        }
        let mut wbi_key = self.wbi_key.write().await;
        let nav_info: serde_json::Value = self
            .get("https://api.bilibili.com/x/web-interface/nav", None)
            .await?
            .json()
            .await?;
        let re = Regex::new(r"wbi/(.*).png").unwrap();
        let img = re
            .captures(nav_info["data"]["wbi_img"]["img_url"].as_str().unwrap())
            .unwrap()
            .get(1)
            .unwrap()
            .as_str();
        let sub = re
            .captures(nav_info["data"]["wbi_img"]["sub_url"].as_str().unwrap())
            .unwrap()
            .get(1)
            .unwrap()
            .as_str();
        let raw_string = format!("{}{}", img, sub);
        *wbi_key = Some(raw_string.clone());
        Ok(raw_string)
    }

    pub async fn get_sign(&self, mut parameters: serde_json::Value) -> FelgensResult<String> {
        let table = vec![
            46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42,
            19, 29, 28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60,
            51, 30, 4, 22, 25, 54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
        ];
        let raw_string = self.get_wbi_key().await?;
        let mut encoded = Vec::new();
        table.into_iter().for_each(|x| {
            if x < raw_string.len() {
                encoded.push(raw_string.as_bytes()[x]);
            }
        });
        // only keep 32 bytes of encoded
        encoded = encoded[0..32].to_vec();
        let encoded = String::from_utf8(encoded).unwrap();
        // Timestamp in seconds
        let wts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        parameters
            .as_object_mut()
            .unwrap()
            .insert("wts".to_owned(), serde_json::Value::String(wts.to_string()));
        // Get all keys from parameters into vec
        let mut keys = parameters
            .as_object()
            .unwrap()
            .keys()
            .map(|x| x.to_owned())
            .collect::<Vec<String>>();
        // sort keys
        keys.sort();
        let mut params = String::new();
        keys.iter().for_each(|x| {
            params.push_str(x);
            params.push('=');
            // Value filters !'()* characters
            let value = parameters
                .get(x)
                .unwrap()
                .as_str()
                .unwrap()
                .replace(['!', '\'', '(', ')', '*'], "");
            let value = PctString::encode(value.chars(), URIReserved);
            params.push_str(value.as_str());
            // add & if not last
            if x != keys.last().unwrap() {
                params.push('&');
            }
        });
        // md5 params+encoded
        let w_rid = md5::compute(params.to_string() + encoded.as_str());
        let params = params + format!("&w_rid={:x}", w_rid).as_str();
        Ok(params)
    }
}

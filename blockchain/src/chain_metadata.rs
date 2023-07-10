use std::collections::HashMap;
use serde_json::Value;
use crate::shared::*;

pub struct ChainsMetadata {

    url: String,
    raw_data: Option<HashMap<u64, HashMap<String,Value>>>
}

impl ChainsMetadata {

    pub fn new(url: String) -> Self {
        Self {url: url, raw_data: None}
    }

    pub async fn download(&mut self) -> Result<()> {
        let contents = reqwest::get(&self.url).await?.text().await?;
        let raw_data_array: Vec<HashMap<String,Value>> = serde_json::from_str(&contents).unwrap();
        let mut raw_data: HashMap<u64, HashMap<String,Value>> = HashMap::new();
        for chain_info in raw_data_array.iter() {
            let chain_id = chain_info["chainId"].as_u64().unwrap();
            raw_data.insert(chain_id, chain_info.clone());
        }
        self.raw_data = Some(raw_data);
        Ok(())
    }

    pub fn get_symbol(&self, chain_id: u64) -> Option<&str> {
        self.get_chain_info(chain_id)?.get("nativeCurrency")?.as_object()?.get("symbol")?.as_str()
    }

    pub fn get_decimals(&self, chain_id: u64) -> Option<u64> {
        self.get_chain_info(chain_id)?.get("nativeCurrency")?.as_object()?.get("decimals")?.as_u64()
    }

    pub fn empty(&self) -> bool {
        self.raw_data.is_none()
    }

    fn get_chain_info(&self, chain_id: u64) -> Option<&HashMap<String, Value>> {
        if let Some(raw_data) = self.raw_data.as_ref() {
            return raw_data.get(&chain_id);
        }
        None
    }

}

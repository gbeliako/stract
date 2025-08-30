// Stract is an open source web search engine.
// Copyright (C) 2024 Stract ApS
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use reqwest::Client;
use serde::Deserialize;
use url::Url;
use anyhow::anyhow;

use crate::{inverted_index::RetrievedWebpage, Result};

#[derive(Debug, Deserialize)]
struct MullvadLetaResult {
    title: String,
    link: String,
    snippet: String,
}

#[derive(Debug, Deserialize)]
struct MullvadLetaResponse {
    items: Vec<MullvadLetaResult>,
}

pub struct MullvadLetaClient {
    client: Client,
    base_url: String,
}

impl MullvadLetaClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://leta.mullvad.net/search".to_string(),
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<RetrievedWebpage>> {
       let url = format!("{}/__data.json", self.base_url);
        let params = [
            ("q", query),
            ("engine", "google"),
            ("x-sveltekit-invalidated", "001"),
        ];

        let response = self.client.get(&url).query(&params).send().await?;
        let json: serde_json::Value = response.json().await?;
        
        // Create an empty vector and use its reference
        let empty_vec = Vec::new();
        
        let items = json["nodes"][2]["data"]
            .as_array()
            .and_then(|data| {
                let items_ptr = data[0]["items"].as_u64()?;
                data[items_ptr as usize].as_array()
            })
            .unwrap_or(&empty_vec); // Use reference to the empty vector

        let mut results = Vec::new();
        for item_ptr in items {
            if let Some(idx) = item_ptr.as_u64() {
                let item_data = json["nodes"][2]["data"][idx as usize].as_object()
                    .ok_or_else(|| anyhow::anyhow!("Expected object at index {}", idx))?;

                let title_idx = item_data["title"].as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Missing title index"))?;
                let link_idx = item_data["link"].as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Missing link index"))?;
                let snippet_idx = item_data["snippet"].as_u64()
                    .ok_or_else(|| anyhow::anyhow!("Missing snippet index"))?;

                let title = json["nodes"][2]["data"][title_idx as usize].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing title string"))?.to_string();
                let url = json["nodes"][2]["data"][link_idx as usize].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing url string"))?.to_string();
                let snippet = json["nodes"][2]["data"][snippet_idx as usize].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing snippet string"))?.to_string();

                results.push(RetrievedWebpage {
                    title,
                    url,
                    body: snippet.clone(),
                    snippet: crate::snippet::TextSnippet {
                        fragments: vec![crate::highlighted::HighlightedFragment::new_unhighlighted(snippet)],
                    },
                    ..Default::default()
                });
            }
        } 
        Ok(results)
    }
}

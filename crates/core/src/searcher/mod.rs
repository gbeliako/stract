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

//! Searchers are responsible for executing search queries against an index.
//! There are two types of searchers:
//! - [`local::LocalSearcher`] which runs the search on the local machine.
//! - [`distributed::DistributedSearcher`] which runs the search on a remote cluster. Each node
//!     will run a local searcher and then the results are merged on the coordinator node.

pub mod api;
pub mod distributed;
pub mod local;
pub mod publicsearch;

pub use distributed::*;
pub use local::*;
use optics::{HostRankings, Optic};

use utoipa::ToSchema;

use crate::api::search::ReturnBody;

use crate::{
    bangs::BangHit,
    collector::approx_count::Count,
    config::defaults,
    ranking::{pipeline::LocalRecallRankingWebpage, SignalCoefficients},
    search_prettifier::DisplayedWebpage,
    webpage::region::Region,
};

pub const NUM_RESULTS_PER_PAGE: usize = 20;

#[derive(Debug, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode)]
pub enum SearchResult {
    Websites(WebsitesResult),
    Bang(Box<BangHit>),
}

#[cfg(test)]
impl SearchResult {
    /// Panics if the result is not a `WebsitesResult`.
    pub fn into_websites_result(self) -> WebsitesResult {
        match self {
            Self::Websites(result) => result,
            _ => panic!("Expected WebsitesResult"),
        }
    }
}

#[derive(
    Debug, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode, ToSchema,
)]
#[serde(rename_all = "camelCase")]
pub struct WebsitesResult {
    pub webpages: Vec<DisplayedWebpage>,
    pub num_hits: Count,
    pub search_duration_ms: u128,
    pub has_more_results: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, bincode::Encode, bincode::Decode, Clone)]
pub struct SearchQuery {
    pub query: String,
    pub page: usize,
    pub num_results: usize,
    pub selected_region: Option<Region>,
    pub optic: Option<Optic>,
    pub host_rankings: Option<HostRankings>,
    pub return_ranking_signals: bool,
    pub safe_search: bool,
    pub count_results_exact: bool,
    pub return_body: Option<ReturnBody>,
    pub return_structured_data: bool,

    pub signal_coefficients: SignalCoefficients,
}

#[cfg(test)]
impl From<String> for SearchQuery {
    fn from(query: String) -> Self {
        Self {
            query,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, bincode::Encode, bincode::Decode)]
pub struct InitialWebsiteResult {
    pub num_websites: Count,
    pub websites: Vec<LocalRecallRankingWebpage>,
}

impl Default for SearchQuery {
    fn default() -> Self {
        // This does not use `..Default::default()` as there should be
        // an explicit compile error when new fields are added to the `SearchQuery` struct
        // to ensure the developer considers what the default should be.
        Self {
            query: Default::default(),
            page: Default::default(),
            num_results: NUM_RESULTS_PER_PAGE,
            selected_region: Default::default(),
            optic: Default::default(),
            host_rankings: Default::default(),
            return_ranking_signals: defaults::SearchQuery::return_ranking_signals(),
            safe_search: defaults::SearchQuery::safe_search(),
            count_results_exact: defaults::SearchQuery::count_results_exact(),
            return_body: None,
            return_structured_data: defaults::SearchQuery::return_structured_data(),
            signal_coefficients: Default::default(),
        }
    }
}

impl SearchQuery {
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }

    pub fn signal_coefficients(&self) -> SignalCoefficients {
        self.signal_coefficients.clone()
    }

    pub fn host_rankings(&self) -> HostRankings {
        let mut rankings = HostRankings::empty();

        if let Some(host_rankings) = &self.host_rankings {
            rankings.merge_into(host_rankings.clone());
        }

        if let Some(optic) = &self.optic {
            rankings.merge_into(optic.host_rankings.clone());
        }

        rankings
    }

    pub fn fetch_backlinks(&self) -> bool {
        let host_rankings = self.host_rankings();
        !host_rankings.liked.is_empty() || !host_rankings.disliked.is_empty()
    }

    pub fn text(&self) -> &str {
        &self.query
    }

    pub fn offset(&self) -> usize {
        self.page * self.num_results
    }

    pub fn num_results(&self) -> usize {
        self.num_results
    }
}

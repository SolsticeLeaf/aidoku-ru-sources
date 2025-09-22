use aidoku::{
    error::{AidokuError, AidokuErrorKind, NodeError, Result},
    prelude::*,
    std::{defaults::defaults_get, net::{HttpMethod, Request}, String},
    Manga, MangaPageResult,
};
use alloc::vec::Vec;

use crate::{
    constants::{BASE_URL, SEARCH_OFFSET_STEP},
    wrappers::WNode,
};

pub fn get_base_url() -> String {
    defaults_get("baseUrl")
        .and_then(|v| v.as_string().ok())
        .map(|s| s.read())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| BASE_URL.to_string())
}

pub fn get_html(url: &str) -> Result<WNode> {
	Request::new(url, HttpMethod::Get)
  	.header("Referer", "https://www.google.com/")
		.html()
		.map(WNode::from_node)
}

pub fn get_manga_url(id: &str) -> String {
    format!("{}/{}", get_base_url(), id)
}

pub fn create_manga_page_result(mangas: Vec<Manga>) -> MangaPageResult {
	let has_more = mangas.len() == SEARCH_OFFSET_STEP as usize;
	MangaPageResult {
		manga: mangas,
		has_more,
	}
}

pub fn get_chapter_url(manga_id: &str, chapter_id: &str) -> String {
	// mtr is 18+ skip
    format!("{}/{manga_id}/{chapter_id}?mtr=true", get_base_url())
}

pub fn create_parsing_error() -> AidokuError {
	AidokuError {
		reason: AidokuErrorKind::NodeError(NodeError::ParseError),
	}
}

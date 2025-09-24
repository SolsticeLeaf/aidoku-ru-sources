use aidoku::MangaStatus;
use aidoku::{
	helpers::uri::encode_uri,
	prelude::*,
	std::{current_date, String, StringRef, Vec},
	Chapter, Filter, FilterType, Manga, MangaContentRating, MangaViewer, Page,
};

extern crate alloc;
use alloc::string::ToString;

use crate::{
	constants::PAGE_DIR,
	helpers::{get_base_url, get_manga_id, get_manga_thumb_url, get_manga_url, parse_status},
	wrappers::WNode,
};

pub fn parse_manga_list(html: &WNode) -> Option<Vec<Manga>> {
	let mut mangas = Vec::new();

	for card_node in html.select("div.cards") {
		let card_mangas = card_node
			.select("a.cards__item")
			.iter()
			.inspect(|node| println!("Found cards__item: {:?}", node.attr("class")))
			.filter(|node| {
				node.attr("class")
					.is_none_or(|class| !class.contains("cloned"))
			})
			.filter_map(|manga_node| {
				let main_node = manga_node;

				let url = main_node.attr("href")?.to_string();
				let img_style = main_node
					.select_one("div.cards__img")?
					.attr("style")?
					.to_string();

				let id = get_manga_id(&url)?;
				let cover = get_manga_thumb_url(&img_style)?;
				let title_node = main_node.select_one("div.cards__name")?;

				Some(Manga {
					id,
					cover,
					title: title_node.text(),
					url,
					nsfw: MangaContentRating::default(),
					..Default::default()
				})
			})
			.collect::<Vec<_>>();

		mangas.extend(card_mangas);
	}

	if mangas.is_empty() {
		None
	} else {
		Some(mangas)
	}
}

pub fn parse_manga(html: &WNode, id: String) -> Option<Manga> {
	let main_node = html.select_one("div.manga")?;
	let description_node = main_node.select_one("div.manga__description")?;

	let image_url = main_node
		.select_one("div.manga__img")?
		.select_one("img")?
		.attr("src")?
		.to_string();
	let cover = format!("{}{}", get_base_url(), image_url);
	let url = get_manga_url(&id);
	let title = main_node.select_one("h1.manga__name")?.text().to_string();

	let categories = main_node
		.select_one("div.tags")
		.map(|type_node| {
			type_node
				.select("a")
				.iter()
				.map(WNode::text)
				.map(|s| s.trim().to_string())
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();

	let status = main_node
		.select_one("div.manga__middle div.manga__middle-links")
		.and_then(|links| {
			links
				.select("a.manga__middle-link")
				.iter()
				.find(|link| {
					link.attr("href")
						.is_some_and(|href| href.contains("status_id"))
				})
				.map(|link| parse_status(link.text().trim()))
		})
		.unwrap_or(MangaStatus::Unknown);

	let viewer = main_node
		.select_one("div.manga__middle div.manga__middle-links")
		.and_then(|links| {
			links
				.select("a.manga__middle-link")
				.iter()
				.find(|link| {
					link.attr("href")
						.is_some_and(|href| href.contains("/types/"))
				})
				.map(|link| match link.text().trim() {
					"Манхва" => MangaViewer::Scroll,
					"OEL-манга" => MangaViewer::Scroll,
					"Комикс Западный" => MangaViewer::Ltr,
					"Маньхуа" => MangaViewer::Scroll,
					"Манга" => MangaViewer::default(),
					_ => MangaViewer::default(),
				})
		})
		.unwrap_or(MangaViewer::default());

	let description = description_node.text().to_string();

	Some(Manga {
		id,
		cover,
		title,
		author: "".to_string(),
		artist: "".to_string(),
		description,
		url,
		categories,
		status,
		nsfw: MangaContentRating::default(),
		viewer,
	})
}

pub fn parse_chapters(html: &WNode, manga_id: &str) -> Option<Vec<Chapter>> {
	let chapter_nodes = html
		.select_one(
			"div.tabs__content div.tabs_page[data-page=chapters] div.chapters div.chapters_list",
		)
		.map(|list| list.select("a.chapters_item"))
		.unwrap_or_default();

	let chapters = chapter_nodes
		.into_iter()
		.enumerate()
		.filter_map(|(idx, chapter_node)| {
			let url = chapter_node.attr("href")?.to_string();
			let id = url
				.trim_start_matches(&format!("{}/", get_manga_url(manga_id)))
				.trim_end_matches('/')
				.to_string();
			let title = chapter_node
				.select_one("div.chapters_name")
				.map(|name| name.text().trim().to_string())
				.unwrap_or_else(|| {
					chapter_node
						.select_one("div.chapters__value span")
						.map(|val| val.text().trim().to_string())
						.unwrap_or_else(|| format!("Глава {}", idx + 1))
				});

			let chapter = chapter_node
				.attr("data-chapter")
				.and_then(|ch| ch.parse::<f32>().ok())
				.unwrap_or_else(|| (idx + 1) as f32);

			let date_updated = chapter_node
				.attr("data-chapter-date")
				.map(|date_str| {
					let parsed = StringRef::from(date_str.trim()).as_date("dd.MM.yyyy", None, None);
					if parsed > 0.0 {
						parsed
					} else {
						current_date()
					}
				})
				.unwrap_or(current_date());

			Some(Chapter {
				id,
				title,
				chapter,
				date_updated,
				url,
				lang: "ru".to_string(),
				..Default::default()
			})
		})
		.collect();

	Some(chapters)
}

pub fn get_page_list(html: &WNode) -> Option<Vec<Page>> {
	let reader_content_node = html.select_one("div.read-container > div.reading-content")?;
	let page_nodes = reader_content_node.select("div.page-break > img");
	let urls: Vec<_> = page_nodes
		.into_iter()
		.filter_map(|img_node| img_node.attr("src"))
		.map(|url| url.trim().to_string())
		.collect();

	Some(
		urls.into_iter()
			.enumerate()
			.map(|(idx, url)| Page {
				index: idx as i32,
				url,
				..Default::default()
			})
			.collect(),
	)
}

pub fn get_filter_url(filters: &[Filter], page: i32) -> Option<String> {
	const QUERY_PART: &str = "&s=";

	let filter_addition: String = filters
		.iter()
		.filter_map(|filter| match filter.kind {
			FilterType::Title => {
				let value = filter.value.clone().as_string().ok()?.read();
				Some(format!("{QUERY_PART}{}", encode_uri(value)))
			}
			_ => None,
		})
		.collect();

	let filter_addition = match filter_addition.find(QUERY_PART) {
		Some(_) => filter_addition,
		None => filter_addition + QUERY_PART,
	};

	Some(format!(
		"{}/{PAGE_DIR}/{page}/?post_type=wp-manga&m_orderby=latest{}",
		get_base_url(),
		filter_addition
	))
}

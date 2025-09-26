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
	println!("parse_manga: start id={}", id);
	let main_node = html.select_one("div.manga")?;
	println!("parse_manga: found main_node");
	let description_node =
		html.select_one("div.tabs__content div.tabs__page[data-page=info] div.manga__description")?;
	println!("parse_manga: found description_node");

	let img_block = if let Some(n) = main_node.select_one("div.manga__img") {
		println!("parse_manga: found manga__img block (within main_node)");
		n
	} else if let Some(n) = html.select_one("div.manga__img") {
		println!("parse_manga: found manga__img block (fallback from root)");
		n
	} else {
		println!("parse_manga: missing manga__img block, will try og:image meta");
		// Try to use og:image as ultimate fallback
		if let Some(meta) = html.select_one("meta[property=og:image]") {
			if let Some(og_image) = meta.attr("content") {
				println!("parse_manga: using og:image={}", og_image);
				let cover = og_image;
				let url = get_manga_url(&id);
				let title = main_node.select_one("h1.manga__name")?.text().to_string();
				println!("parse_manga(og): url={}", url);
				println!("parse_manga(og): title={}", title);
				let categories = html
					.select_one("div.tags")
					.map(|type_node| {
						type_node
							.select("a.tags__item")
							.iter()
							.map(WNode::text)
							.map(|s| s.trim().to_string())
							.collect::<Vec<_>>()
					})
					.unwrap_or_default();
				println!("parse_manga(og): categories={:?}", categories);
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
				println!("parse_manga(og): status={:?}", status);
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
				println!("parse_manga(og): viewer={:?}", viewer);
				let description = description_node.text().to_string();
				println!("parse_manga(og): description.len={}", description.len());
				return Some(Manga {
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
				});
			}
		}
		return None;
	};
	let img_node = match img_block.select_one("img") {
		Some(n) => {
			println!("parse_manga: found <img> inside manga__img");
			n
		}
		None => {
			println!("parse_manga: missing <img> in manga__img");
			return None;
		}
	};
	let image_url = match img_node.attr("src").or_else(|| img_node.attr("data-src")) {
		Some(s) => s,
		None => {
			println!("parse_manga: image has no src or data-src");
			return None;
		}
	};
	println!("parse_manga: image_url={}", image_url);
	let cover = format!("{}{}", get_base_url(), image_url);
	println!("parse_manga: cover={}", cover);
	let url = get_manga_url(&id);
	println!("parse_manga: url={}", url);
	let title = main_node.select_one("h1.manga__name")?.text().to_string();
	println!("parse_manga: title={}", title);

	let categories = html
		.select_one("div.tags")
		.map(|type_node| {
			type_node
				.select("a.tags__item")
				.iter()
				.map(WNode::text)
				.map(|s| s.trim().to_string())
				.collect::<Vec<_>>()
		})
		.unwrap_or_default();
	println!("parse_manga: categories={:?}", categories);

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
	println!("parse_manga: status={:?}", status);

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
	println!("parse_manga: viewer={:?}", viewer);

	let description = description_node.text().to_string();
	println!("parse_manga: description.len={}", description.len());

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
	println!("parse_chapters: start manga_id={}", manga_id);
	let chapter_nodes = html
		.select_one(
			"div.tabs__content div.tabs__page[data-page=chapters] div.chapters div.chapters__list",
		)
		.map(|list| list.select("a.chapters__item"))
		.unwrap_or_default();
	println!(
		"parse_chapters: found {} chapter nodes",
		chapter_nodes.len()
	);

	let chapters = chapter_nodes
		.into_iter()
		.enumerate()
		.filter_map(|(idx, chapter_node)| {
			if chapter_node.attr("href").is_none() {
				println!("parse_chapters[{}]: missing href", idx);
				return None;
			}
			let url = chapter_node.attr("href")?.to_string();
			println!("parse_chapters[{}]: url={}", idx, url);
			let id = url
				.trim_start_matches(&format!("{}/", get_manga_url(manga_id)))
				.trim_end_matches('/')
				.to_string();
			println!("parse_chapters[{}]: id={}", idx, id);
			let title = chapter_node
				.select_one("div.chapters__name")
				.map(|name| {
					let t = name.text().trim().to_string();
					if t.is_empty() {
						chapter_node
							.select_one("div.chapters__value span")
							.map(|val| val.text().trim().to_string())
							.unwrap_or_else(|| format!("Глава {}", idx + 1))
					} else {
						t
					}
				})
				.unwrap_or_else(|| {
					chapter_node
						.select_one("div.chapters__value span")
						.map(|val| val.text().trim().to_string())
						.unwrap_or_else(|| format!("Глава {}", idx + 1))
				});
			println!("parse_chapters[{}]: title={}", idx, title);

			let chapter = chapter_node
				.attr("data-chapter")
				.and_then(|ch| ch.parse::<f32>().ok())
				.unwrap_or_else(|| (idx + 1) as f32);
			println!("parse_chapters[{}]: chapter={}", idx, chapter);

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
			println!("parse_chapters[{}]: date_updated={}", idx, date_updated);

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
	let reader_content_node = html.select_one("ul.pagination")?;
	let page_nodes = reader_content_node.select("li.pagination__button");
	let urls: Vec<_> = page_nodes.into_iter().map(|url| url.text()).collect();

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
	const QUERY_PART: &str = "&q=";

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
		"{}/search?type=manga&page={}{}",
		get_base_url(),
		page,
		filter_addition
	))
}

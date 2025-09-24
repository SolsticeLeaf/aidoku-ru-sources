use aidoku::MangaStatus;
use aidoku::{
	helpers::uri::encode_uri,
	prelude::*,
	std::{current_date, String, StringRef, Vec},
	Chapter, Filter, FilterType, Manga, MangaContentRating, MangaViewer, Page,
};

extern crate alloc;
use alloc::string::ToString;
use core::cmp::Ordering;

use crate::{
	constants::PAGE_DIR,
	helpers::{get_base_url, get_manga_id, get_manga_thumb_url, get_manga_url, parse_status},
	wrappers::{post, WNode},
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
	let description_node = html.select_one("div.description-summary")?;
	let summary_node = main_node.select_one("div.tab-summary")?;
	let summary_content_node = summary_node.select_one("div.summary_content")?;
	let content_node = summary_content_node.select_one("div.post-content")?;

	let extract_optional_content = |content_type| {
		content_node
			.select_one(&format!("div.{}-content", content_type))
			.map(|type_node| {
				type_node
					.select("a")
					.iter()
					.map(WNode::text)
					.map(|s| s.trim().to_string())
					.collect::<Vec<_>>()
			})
			.unwrap_or_default()
	};

	let get_row_value_by_name = |parent_node: &WNode, row_name| {
		parent_node
			.select("div.post-content_item")
			.iter()
			.find(|n| {
				n.select_one("div.summary-heading")
					.is_some_and(|heading| heading.text().trim() == row_name)
			})
			.and_then(|n| {
				n.select_one("div.summary-content")
					.map(|c| c.text().trim().to_string())
			})
	};

	let cover = summary_node
		.select_one("div.summary_image img")?
		.attr("data-src")
		.or_else(|| {
			summary_node
				.select_one("div.summary_image img")?
				.attr("src")
		})?;
	let url = get_manga_url(&id);
	let title = main_node
		.select_one("div.post-title > h1")?
		.text()
		.trim()
		.to_string();
	let author = extract_optional_content("authors").join(", ");
	let artist = extract_optional_content("artist").join(", ");

	let categories = extract_optional_content("genres");
	let status = get_row_value_by_name(&content_node, "Статус")
		.map(|status_str| parse_status(&status_str))
		.unwrap_or(MangaStatus::Unknown);
	let viewer = get_row_value_by_name(&content_node, "Тип")
		.map(|manga_type| match manga_type.trim() {
			"Манхва" | "Маньхуа" => MangaViewer::Scroll,
			_ => MangaViewer::default(),
		})
		.unwrap_or(MangaViewer::default());
	let description = description_node.text().trim().to_string();

	Some(Manga {
		id,
		cover,
		title,
		author,
		artist,
		description,
		url,
		categories,
		status,
		nsfw: MangaContentRating::default(),
		viewer,
	})
}

pub fn parse_chapters(html: &WNode, manga_id: &str) -> Option<Vec<Chapter>> {
	let chapter_nodes =
		match html.select_one("div.page-content-listing.single-page ul.main.version-chap") {
			Some(list) => list.select("li.wp-manga-chapter"),
			None => {
				let manga_chapters_holder_node =
					html.select_one("div.c-page-content div#manga-chapters-holder")?;
				let data_id = manga_chapters_holder_node.attr("data-id")?;
				let real_manga_chapters_holder_node = post(
					&format!("{}/wp-admin/admin-ajax.php", get_base_url()),
					&format!("action=manga_get_chapters&manga={data_id}"),
					&[
						("X-Requested-With", "XMLHttpRequest"),
						("Referer", &format!("{}", get_manga_url(manga_id))),
					],
				)
				.ok()?;
				real_manga_chapters_holder_node.select("ul > li.wp-manga-chapter")
			}
		};

	let chapters = chapter_nodes
		.into_iter()
		.enumerate()
		.filter_map(|(idx, chapter_node)| {
			let url_node = chapter_node.select_one("a")?;
			let url = url_node.attr("href")?.to_string();
			let id = url
				.trim_start_matches(&format!("{}/", get_manga_url(manga_id)))
				.trim_end_matches('/')
				.to_string();
			let title = url_node.text().trim().to_string();

			let chapter = {
				let approx_chapter = (idx + 1) as f32;
				let possible_chapters: Vec<_> = title
					.split_whitespace()
					.filter_map(|word| word.parse::<f32>().ok())
					.collect();
				match possible_chapters.as_slice() {
					[] => approx_chapter,
					[chap] => *chap,
					_ => possible_chapters
						.iter()
						.min_by(|&&l, &&r| {
							let l_diff = (l - approx_chapter).abs();
							let r_diff = (r - approx_chapter).abs();
							l_diff.partial_cmp(&r_diff).unwrap_or(Ordering::Equal)
						})
						.copied()
						.unwrap_or(approx_chapter),
				}
			};

			let date_updated = {
				let release_date_node = chapter_node.select_one("span.chapter-release-date")?;
				release_date_node
					.select_one("i")
					.map(|i_node| {
						let text = i_node.text();
						let txt = text.trim();
						let parsed1 = StringRef::from(txt).as_date("dd.MM.yyyy", None, None);
						if parsed1 > 0.0 {
							parsed1
						} else {
							StringRef::from(txt).as_date("dd-MM-yyyy", None, None)
						}
					})
					.unwrap_or_else(|| {
						release_date_node
							.select_one("a")
							.and_then(|a| a.attr("title"))
							.and_then(|updated_text| {
								if !updated_text.ends_with("ago") {
									return None;
								}
								let spl: Vec<&str> = updated_text.split_whitespace().collect();
								let count = spl.first().and_then(|s| s.parse::<f64>().ok())?;
								let metric_mult = spl.get(1).and_then(|&metric| match metric {
									"сек" => Some(1.0),
									"мин" => Some(60.0),
									"час" => Some(3600.0),
									"дн" => Some(86400.0),
									_ => None,
								})?;
								let current_time = current_date(); // 24.09.2025 19:40 CEST
								Some(current_time - count * metric_mult)
							})
							.unwrap_or(0.0)
					})
			};

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

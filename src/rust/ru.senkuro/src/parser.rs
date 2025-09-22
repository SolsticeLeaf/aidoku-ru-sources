use aidoku::{
	helpers::{substring::Substring, uri::encode_uri},
	prelude::*,
	std::{current_date, String, StringRef, Vec},
	Chapter, DeepLink, Filter, FilterType, Manga, MangaContentRating, MangaViewer, Page,
};

extern crate alloc;
use alloc::string::ToString;

use crate::{
	constants::{BASE_SEARCH_URL, BASE_URL},
	helpers::{get_manga_url, parse_status},
	sorting::Sorting,
	wrapper::{post, WNode},
};

pub fn parse_search_results(html: &WNode) -> Result<Vec<Manga>> {
	let list_node = html
		.select_one("div.c-page-content div.main-col-inner div.tab-content-wrap div.c-tabs-item")?;

	let mangas = list_node
		.select("div.row.c-tabs-item__content")
		.iter()
		.filter_map(|manga_node| {
			let thumb_node = manga_node.select_one("div.tab-thumb")?;
			let summary_node = manga_node.select_one("div.tab-summary")?;

			let title_node = summary_node.select_one("div.post-title a")?;
			let content_node = summary_node.select_one("div.post-content")?;

			let extract_from_content = |class_name: &str| {
				content_node
					.select_one(&format!("div.{class_name}"))?
					.select_one("div.summary-content")?
					.text()
			};

			let url = title_node.attr("href")?;
			let id = url.substring_after_last("/")?.to_string();
			let cover = thumb_node.select_one("img")?.attr("src")?;
			let title = title_node.text();
			let author = extract_from_content("mg_author");
			let artist = extract_from_content("mg_artist");
			let categories: Vec<String> = content_node
				.select("div.mg_genres a")
				.iter()
				.map(WNode::text)
				.collect();
			let status = parse_status(&extract_from_content("mg_status"));
			let nsfw = if categories.iter().any(|c| c.contains("18+")) {
				MangaContentRating::Nsfw
			} else {
				MangaContentRating::Suggestive
			};

			Some(Manga {
				id,
				cover,
				title,
				author,
				artist,
				url,
				categories,
				status,
				nsfw,
				..Default::default()
			})
		})
		.collect::<Vec<_>>();

	Ok(mangas)
}

pub fn parse_manga(html: &WNode, id: String) -> Result<Manga> {
	let main_node = html.select_one("div.profile-manga > div.container > div.row")?;

	let description_node = html.select_one("div.c-page-content div.description-summary")?;
	let summary_node = main_node.select_one("div.tab-summary")?;
	let summary_content_node = summary_node.select_one("div.summary_content")?;
	let content_node = summary_content_node.select_one("div.post-content")?;
	let status_node = summary_content_node.select_one("div.post-status")?;

	let extract_optional_content = |content_type: &str| {
		content_node
			.select_one(&format!("div.{content_type}-content"))
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

	let get_row_value_by_name = |parent_node: &WNode, row_name: &str| {
		parent_node
			.select("div.post-content_item")
			.iter()
			.find(|n| n.select_one("div.summary-heading")?.text().trim() == row_name)
			.and_then(|n| Some(n.select_one("div.summary-content")?.text().trim().to_string()))
	};

	let cover = summary_node.select_one("div.summary_image img")?.attr("src")?;
	let url = get_manga_url(&id);
	let title = main_node.select_one("div.post-title > h1")?.text();
	let author = extract_optional_content("authors").join(", ");
	let artist = extract_optional_content("artist").join(", ");

	let categories = extract_optional_content("genres");
	let nsfw = if categories.iter().any(|c| c.contains("18+")) {
		MangaContentRating::Nsfw
	} else {
		MangaContentRating::Suggestive
	};
	let status = get_row_value_by_name(&status_node, "Статус")
		.map(|status_str| parse_status(&status_str))
		.unwrap_or(MangaStatus::Unknown);
	let viewer = get_row_value_by_name(&content_node, "Тип")
		.map(|manga_type| match manga_type.as_str() {
			"Манхва" => MangaViewer::Scroll,
			"Маньхуа" => MangaViewer::Scroll,
			_ => MangaViewer::default(),
		})
		.unwrap_or_default();
	let description = description_node.text();

	Ok(Manga {
		id,
		cover,
		title,
		author,
		artist,
		description,
		url,
		categories,
		status,
		nsfw,
		viewer,
	})
}

pub fn parse_chapters(html: &WNode, manga_id: &str) -> Result<Vec<Chapter>> {
	let manga_chapters_holder_node = html.select_one("div.c-page-content div#manga-chapters-holder")?;

	let data_id = manga_chapters_holder_node.attr("data-id")?;

	let real_manga_chapters_holder_node = post(
		&format!("{BASE_URL}/wp-admin/admin-ajax.php"),
		&format!("action=manga_get_chapters&manga={data_id}"),
		&[
			("X-Requested-With", "XMLHttpRequest"),
			("Referer", &format!("{}", get_manga_url(manga_id))),
			("Content-Type", "application/x-www-form-urlencoded"),
		],
	)?;

	let chapter_nodes = real_manga_chapters_holder_node.select("ul > li.wp-manga-chapter");

	let abs = |l: f32, r: f32| if l > r { l - r } else { r - l };

	let chapters = chapter_nodes
		.iter()
		.enumerate()
		.filter_map(|(idx, chapter_node| {
			let url_node = chapter_node.select_one("a")?;
			let url = url_node.attr("href")?;
			let id = url.substring_after(&format!("{}/", get_manga_url(manga_id)))?.trim_end_matches('/').to_string();
			let title = url_node.text();

			let chapter = {
				let approx_chapter = (idx + 1) as f32;
				let mut possible_chapters: Vec<_> = title
					.split_whitespace()
					.filter_map(|word| word.parse::<f32>().ok())
					.collect();
				match &possible_chapters[..] {
					[] => approx_chapter,
					[chap] => *chap,
					_ => {
						possible_chapters.sort_by(|&l, &r| abs(l, approx_chapter).partial_cmp(&abs(r, approx_chapter)).unwrap());
						*possible_chapters.first()?
					}
				}
			};

			let extract_multiplier = |metric_str: &&str| {
				if metric_str.starts_with("сек") {
					Some(1)
				} else if metric_str.starts_with("мин") {
					Some(60)
				} else if metric_str.starts_with("час") {
					Some(60 * 60)
				} else if metric_str.starts_with("дн") {
					Some(24 * 60 * 60)
				} else {
					None
				}
			};

			let date_updated = {
				let release_date_node = chapter_node.select_one("span.chapter-release-date")?;
				let normal_release_date = release_date_node.select_one("i").map(|i_node| {
					StringRef::from(&i_node.text()).as_date("dd-MM-yyyy", None, None)
				});

				let ago_extractor = || {
					release_date_node
						.select_one("a")
						.and_then(|a| a.attr("title"))
						.and_then(|updated_text| {
							if !updated_text.ends_with("ago") {
								return None;
							}
							let spl: Vec<_> = updated_text.split_whitespace().collect();
							let count = spl.first()?.parse::<f64>().ok()?;
							let metric_mult = spl.get(1).and_then(extract_multiplier)?;
							Some(current_date() - count * (metric_mult as f64))
						})
						.unwrap_or(0f64)
				};

				normal_release_date.unwrap_or_else(ago_extractor)
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
		.collect::<Vec<_>>();

	Ok(chapters)
}

pub fn get_page_list(html: &WNode) -> Result<Vec<Page>> {
	let reader_content_node = html.select_one("div.read-container > div.reading-content")?;

	let page_nodes = reader_content_node.select("div.page-break > img");

	let urls: Vec<_> = page_nodes
		.iter()
		.filter_map(|img_node| img_node.attr("src"))
		.map(|url| url.trim().to_string())
		.collect();

	Ok(urls
		.into_iter()
		.enumerate()
		.map(|(idx, url)| Page {
			index: idx as i32,
			url,
			..Default::default()
		})
		.collect())
}

pub fn get_filter_url(filters: &[Filter], sorting: &Sorting, page: i32) -> Result<String> {
	let filter_addition: String = filters
		.iter()
		.filter_map(|filter| match filter.kind {
			FilterType::Title => {
				filter
					.value
					.as_string()
					.ok()
					.map(|v| format!("&s={}", encode_uri(v.read())))
			}
			_ => None,
		})
		.collect();

	let url = format!(
		"{}/page/{}/?m_orderby={}{}",
		BASE_SEARCH_URL, page, sorting, filter_addition
	);

	Ok(url)
}

pub fn parse_incoming_url(url: &str) -> Result<DeepLink> {
	let manga_id = match url.find("://") {
		Some(idx) => &url[idx + 3..],
		None => url,
	}
	.split('/')
	.next()
	.ok_or(AidokuError {
		reason: AidokuErrorKind::Unimplemented,
	})?.to_string();

	Ok(DeepLink {
		manga: Some(get_manga_details(manga_id)?),
		chapter: None,
	})
}
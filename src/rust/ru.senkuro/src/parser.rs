use core::iter::once;

use aidoku::{
	error::{AidokuError, AidokuErrorKind, Result},
	helpers::{substring::Substring, uri::encode_uri},
	prelude::*,
	std::{String, StringRef, Vec},
	Chapter, DeepLink, Filter, FilterType, Manga, MangaContentRating, MangaStatus, MangaViewer,
	Page,
};

extern crate alloc;
use alloc::{boxed::Box, string::ToString};

use itertools::chain;

use crate::{
	constants::{BASE_SEARCH_URL, BASE_URL, SEARCH_OFFSET_STEP},
	get_manga_details, helpers,
	sorting::Sorting,
	wrappers::WNode,
};

pub fn parse_search_results(html: &WNode) -> Result<Vec<Manga>> {
	let nodes = html.select("article.card-main");

	let mangas: Vec<_> = nodes
		.into_iter()
		.filter_map(|node| {
			let a_node = node.select("a").pop()?;
			let href = a_node.attr("href")?;
			// Пример: /manga/my-childhood-friends-are-trying-to-kill-me/chapters
			let id = href
				.trim_start_matches("/manga/")
				.trim_end_matches("/chapters")
				.trim_matches('/')
				.to_string();

			let img_node = a_node.select("img").pop()?;
			let cover = img_node.attr("src")?.to_string();
			let title = node.select("h3.card-title").pop()?.text();

			// Жанры (теги) — все span.tag внутри .card-status__down
			let mut categories = Vec::new();
			if let Some(status_down) = node.select("div.card-status__down").pop() {
				for tag in status_down.select("span.tag") {
					let text = tag.text().trim().to_string();
					if !text.is_empty() && !text.chars().all(|c| c.is_numeric() || c == '.' || c == ',') {
						categories.push(text);
					}
				}
			}

			// Оценка (рейтинг) можно парсить отдельно, если нужно
			// let rating = ...

			let url = helpers::get_manga_url(&id);

			Some(Manga {
				id,
				cover,
				title,
				author: String::new(),
				artist: String::new(),
				description: String::new(),
				url,
				categories,
				status: MangaStatus::Unknown,
				nsfw: MangaContentRating::default(),
				viewer: MangaViewer::Rtl,
			})
		})
		.collect();

	Ok(mangas)
}

fn get_manga_page_main_node(html: &WNode) -> Result<WNode> {
	html.select("div.leftContent")
		.pop()
		.ok_or(helpers::create_parsing_error())
}

pub fn parse_manga(html: &WNode, id: String) -> Result<Manga> {
    let parsing_error = helpers::create_parsing_error();

    // Обложка
    let cover = html
        .select(".project-poster img")
        .pop()
        .and_then(|img| img.attr("src"))
        .unwrap_or("")
        .to_string();

    // Название
    let title = html
        .select("h1.caption.caption-size-l")
        .pop()
        .map(|n| n.text())
        .unwrap_or_default();

    // Жанры/лейблы
    let categories: Vec<String> = html
        .select(".project-tags a > span")
        .into_iter()
        .map(|n| n.text().trim().to_string())
        .filter(|s| !s.is_empty() && !s.chars().all(|c| c.is_numeric()))
        .collect();

    // Описание
    let description = html
        .select(".text-expander__collapse p")
        .into_iter()
        .map(|n| n.text().trim().to_string())
        .collect::<Vec<_>>()
        .join("\n");

    // Статус
    let status = html
        .select(".project-stats__item")
        .into_iter()
        .find_map(|item| {
            let label = item.select(".project-stats__text").pop()?.text();
            if label.contains("Статус тайтла") {
                let value = item.select(".project-stats__name").pop()?.text().to_lowercase();
                if value.contains("завершён") { Some(MangaStatus::Completed) }
                else if value.contains("онгоинг") { Some(MangaStatus::Ongoing) }
                else if value.contains("приостановлен") { Some(MangaStatus::Hiatus) }
                else { Some(MangaStatus::Unknown) }
            } else {
                None
            }
        })
        .unwrap_or(MangaStatus::Unknown);

    let url = helpers::get_manga_url(&id);

    Ok(Manga {
        id,
        cover,
        title,
        author: String::new(),
        artist: String::new(),
        description,
        url,
        categories,
        status,
        nsfw: MangaContentRating::default(),
        viewer: MangaViewer::Rtl,
    })
}

pub fn parse_chapters(html: &WNode, manga_id: &str) -> Result<Vec<Chapter>> {
    let chapters = html
        .select(".chapter-result > article.card-chapter")
        .into_iter()
        .filter_map(|chapter_elem| {
            let link_elem = chapter_elem.select("a.card-chapter__link").pop()?;
            let href = link_elem.attr("href")?;
            // Пример: /manga/sinmadaeje/chapters/197198132029703703/pages/1
            let id = href
                .split("/chapters/")
                .nth(1)?
                .split('/')
                .next()?;
            let id = id.to_string();

            let full_title = link_elem.select("h3").pop()?.text();
            let title = full_title
                .split('>').last().unwrap_or(&full_title).trim().to_string();

            // Том и номер главы (можно парсить из текста h3)
            let (volume, chapter) = {
                let text = full_title.replace("Том", "").replace("Глава", "");
                let mut vol = None;
                let mut chap = None;
                for part in text.split_whitespace() {
                    if vol.is_none() && part.chars().all(|c| c.is_digit(10)) {
                        vol = part.parse().ok();
                    } else if chap.is_none() && part.chars().all(|c| c.is_digit(10)) {
                        chap = part.parse().ok();
                    }
                }
                (vol, chap)
            };

            // Дата
            let date_updated = chapter_elem
                .select("span")
                .pop()
                .map(|n| StringRef::from(&n.text()).as_date("yyyy-MM-dd", None, None))
                .unwrap_or(0.0);

            let url = helpers::get_chapter_url(manga_id, &id);

            Some(Chapter {
                id,
                title,
                volume: volume.unwrap_or(0),
                chapter: chapter.unwrap_or(0),
                date_updated,
                scanlator: String::new(),
                url,
                lang: "ru".to_string(),
            })
        })
        .collect();

    Ok(chapters)
}

pub fn get_page_list(html: &WNode) -> Result<Vec<Page>> {
	let parsing_error = helpers::create_parsing_error();

	let script_text = html
		.select(r"div.reader-controller > script[type=text/javascript]")
		.pop()
		.map(|script_node| script_node.data())
		.ok_or(parsing_error)
		.map(|mut text| {
			text.replace_range(0..text.find("rm_h.readerDoInit(").unwrap_or_default(), "");
			text
		})?;

	let chapters_list_str = script_text
		.find("[[")
		.zip(script_text.find("]]"))
		.map(|(start, end)| &script_text[start..end + 2])
		.ok_or(parsing_error)?;

	let urls: Vec<_> = chapters_list_str
		.match_indices("['")
		.zip(chapters_list_str.match_indices("\","))
		.filter_map(|((l, _), (r, _))| {
			use itertools::Itertools;
			chapters_list_str[l + 1..r + 1]
				.replace(['\'', '"'], "")
				.split(',')
				.map(ToString::to_string)
				.collect_tuple()
		})
		.map(|(part0, part1, part2)| {
			if part1.is_empty() && part2.starts_with("/static/") {
				format!("{BASE_URL}{part2}")
			} else if part1.starts_with("/manga/") {
				format!("{part0}{part2}")
			} else {
				format!("{part0}{part1}{part2}")
			}
		})
		.map(|url| {
			if !url.contains("://") {
				format!("https:{url}")
			} else {
				url
			}
		})
		.filter_map(|url| {
			if url.contains("one-way.work") {
				url.substring_before("?").map(ToString::to_string)
			} else {
				Some(url)
			}
		})
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
    fn get_handler(operation: &'static str) -> Box<dyn Fn(AidokuError) -> AidokuError> {
        Box::new(move |err: AidokuError| {
            println!("Error {:?} while {}", err.reason, operation);
            err
        })
    }

    let mut params: Vec<String> = Vec::new();

    params.push(format!("offset={}", (page - 1) * SEARCH_OFFSET_STEP));
    params.push(format!("sortType={}", sorting));

    for filter in filters {
        match filter.kind {
            FilterType::Title => {
                if let Ok(title_ref) = filter.value.clone().as_string() {
                    params.push(format!("q={}", encode_uri(title_ref.read())));
                }
            }
            FilterType::Genre => {
                if let Ok(id_ref) = filter.object.get("id").as_string() {
                    let id = id_ref.read();
                    match filter.value.as_int().unwrap_or(-1) {
                        0 => params.push(format!("{}=out", id)), // excluded
                        1 => params.push(format!("{}=in", id)),  // included
                        _ => {}
                    }
                }
            }
            FilterType::Check => {
                if let Ok(id_ref) = filter.object.get("id").as_string() {
                    let id = id_ref.read();
                    // Any checked option => add `=in`
                    if filter.value.as_int().unwrap_or(0) != 0 {
                        params.push(format!("{}=in", id));
                    }
                }
            }
            _ => {}
        }
    }

    params.sort_by(|a, b| {
        let a_is_q = a.starts_with("q=");
        let b_is_q = b.starts_with("q=");
        b_is_q.cmp(&a_is_q)
    });

    Ok(format!("{}{}", BASE_SEARCH_URL, params.join("&")))
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
	})?;

	Ok(DeepLink {
		manga: Some(get_manga_details(manga_id.to_string())?),
		chapter: None,
	})
}

use const_format::formatcp;

pub const BASE_URL: &str = "https://senkuro.me";
pub const BASE_SEARCH_URL: &str = formatcp!("{}/{}", BASE_URL, "browse/manga/");

pub const SEARCH_OFFSET_STEP: i32 = 50;

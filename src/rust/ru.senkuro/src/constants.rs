use const_format::formatcp;

pub const BASE_URL: &str = "https://senkuro.me";
pub const BASE_SEARCH_URL: &str = formatcp!("{}/?post_type=wp-manga", BASE_URL);

pub const SEARCH_OFFSET_STEP: i32 = 12;
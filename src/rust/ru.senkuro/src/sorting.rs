use core::fmt::Display;

use aidoku::Listing;

#[derive(Debug, Default)]
pub enum Sorting {
	#[default]
	Rating,
	Popular,
	UpdatedRecently,
}

impl Sorting {
	pub fn from_listing(listing: &Listing) -> Self {
		match listing.name.as_str() {
			"Рейтинг" => Self::Rating,
			"Популярное" => Self::Popular,
			"Обновлено недавно" => Self::UpdatedRecently,
			_ => Self::Rating,
		}
	}
}

impl Display for Sorting {
	fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		match self {
			Sorting::Rating => write!(f, "rating"),
			Sorting::Popular => write!(f, "views"),
			Sorting::UpdatedRecently => write!(f, "latest"),
		}
	}
}
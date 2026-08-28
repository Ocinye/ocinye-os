//! Pagination contract, uniform across every collection endpoint.

use serde::{Deserialize, Serialize};

/// Largest page a caller may request.
pub const MAX_PAGE_SIZE: u32 = 100;
/// Page size applied when the caller does not choose one.
pub const DEFAULT_PAGE_SIZE: u32 = 25;

/// Requested page.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PageRequest {
    /// 1-based page number.
    #[serde(default = "default_page")]
    pub page: u32,
    /// Items per page, clamped to [`MAX_PAGE_SIZE`].
    #[serde(default = "default_page_size")]
    pub page_size: u32,
}

const fn default_page() -> u32 {
    1
}

const fn default_page_size() -> u32 {
    DEFAULT_PAGE_SIZE
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl PageRequest {
    /// Clamp caller-supplied values into the permitted range.
    ///
    /// Clamping rather than rejecting keeps a malformed page parameter from
    /// becoming a denial-of-service vector against the database.
    #[must_use]
    pub fn normalised(self) -> Self {
        Self {
            page: self.page.max(1),
            page_size: self.page_size.clamp(1, MAX_PAGE_SIZE),
        }
    }

    /// SQL `LIMIT`.
    #[must_use]
    pub fn limit(self) -> i64 {
        i64::from(self.normalised().page_size)
    }

    /// SQL `OFFSET`.
    #[must_use]
    pub fn offset(self) -> i64 {
        let normalised = self.normalised();
        i64::from(normalised.page - 1) * i64::from(normalised.page_size)
    }
}

/// One page of results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page<T> {
    /// Items on this page.
    pub items: Vec<T>,
    /// 1-based page number.
    pub page: u32,
    /// Items per page.
    pub page_size: u32,
    /// Total items matching the *authorised* query.
    pub total: i64,
    /// Total pages.
    pub total_pages: i64,
}

impl<T> Page<T> {
    /// Assemble a page.
    #[must_use]
    pub fn new(items: Vec<T>, request: PageRequest, total: i64) -> Self {
        let request = request.normalised();
        let page_size = i64::from(request.page_size);
        Self {
            items,
            page: request.page,
            page_size: request.page_size,
            total,
            total_pages: (total + page_size.max(1) - 1) / page_size.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_size_is_clamped() {
        let request = PageRequest {
            page: 0,
            page_size: 10_000,
        }
        .normalised();
        assert_eq!(request.page, 1);
        assert_eq!(request.page_size, MAX_PAGE_SIZE);
    }

    #[test]
    fn offsets_are_computed_from_normalised_values() {
        let request = PageRequest {
            page: 3,
            page_size: 10,
        };
        assert_eq!(request.offset(), 20);
        assert_eq!(request.limit(), 10);
    }

    #[test]
    fn total_pages_rounds_up() {
        let page = Page::new(
            vec![1, 2, 3],
            PageRequest {
                page: 1,
                page_size: 10,
            },
            21,
        );
        assert_eq!(page.total_pages, 3);
    }
}

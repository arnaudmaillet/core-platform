#[derive(Debug, Clone)]
pub struct PageQuery {
    pub limit: usize,
    pub cursor: Option<String>,
}

impl PageQuery {
    pub fn new(limit: usize, cursor: Option<String>) -> Self {
        Self { limit, cursor }
    }
}

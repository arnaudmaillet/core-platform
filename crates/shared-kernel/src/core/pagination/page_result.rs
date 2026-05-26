pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}

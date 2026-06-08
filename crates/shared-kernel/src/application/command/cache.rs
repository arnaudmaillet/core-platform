pub trait CacheableCommand {
    fn cache_key(&self) -> String;
}

pub trait CacheKeyComponent {
    fn to_key_component(&self) -> Option<String>;
}

impl CacheKeyComponent for crate::types::Region {
    fn to_key_component(&self) -> Option<String> {
        Some(self.as_str().to_string())
    }
}

impl CacheKeyComponent for () {
    fn to_key_component(&self) -> Option<String> {
        None
    }
}

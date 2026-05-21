pub trait CacheableCommand {
    fn cache_key(&self) -> String;
}

/// A single GIF attachment from an external provider (Giphy, Tenor, etc.).
///
/// All four fields must be present together — partial GIF metadata is a domain
/// violation detected in [`crate::domain::aggregate::comment::Comment::create`].
#[derive(Debug, Clone)]
pub struct GifAttachment {
    pub gif_id:     String,
    pub gif_url:    String,
    pub gif_width:  u32,
    pub gif_height: u32,
}

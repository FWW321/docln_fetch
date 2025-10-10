#[derive(Debug, Clone)]
pub struct Chapter {
    pub index: usize,
    pub title: String,
    pub url: String,
    pub has_illustrations: bool, // 是否包含插图
    pub images: Vec<String>,     // 章节内的图片列表
    pub filename: String,
}

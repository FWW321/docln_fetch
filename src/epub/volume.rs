use crate::epub::chapter::Chapter;

#[derive(Debug, Clone)]
pub struct Volume {
    // pub title: String,
    pub index: usize,
    pub id: String,
    pub cover: Option<String>,
    pub chapters: Vec<Chapter>,
    pub cover_chapter: Chapter,
}

impl Volume {
    pub fn cover_html(&self) -> String {
        let mut xhtml_content = String::new();

        xhtml_content.push_str(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE html PUBLIC "-//W3C//DTD XHTML 1.1//EN" "http://www.w3.org/TR/xhtml11/DTD/xhtml11.dtd">
<html xmlns="http://www.w3.org/1999/xhtml">
<head>
    <title>"#,
        );

        xhtml_content.push_str(&self.cover_chapter.title);
        xhtml_content.push_str(
            r#"</title>
    <meta http-equiv="Content-Type" content="text/html; charset=UTF-8"/>
</head>
<body>
    <div class="cover">
        <h1>"#,
        );

        xhtml_content.push_str(&self.cover_chapter.title);
        xhtml_content.push_str(
            r#"</h1>
"#,
        );

        // 插入封面图片
        if let Some(cover_name) = &self.cover {
            // 计算相对路径（假设cover_path已是相对OEBPS的路径）
            xhtml_content.push_str(&format!(
                "        <img src=\"../images/{}\" alt=\"封面\" class=\"volume-cover-img\"/>",
                cover_name
            ));
            xhtml_content.push('\n');
        }

        xhtml_content.push_str(
            r#"    </div>
</body>
</html>"#,
        );
        xhtml_content
    }
}

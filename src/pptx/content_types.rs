pub struct ContentTypesXml {
    slides: Vec<(u32, String)>,
}

impl ContentTypesXml {
    pub fn new(slides: &[(u32, String)]) -> Self {
        Self { slides: slides.to_vec() }
    }

    pub fn to_string(&self) -> String {
        let mut entries = String::new();
        // Per-slide Override entries (critical for PPTX validity!)
        for (num, _) in &self.slides {
            entries.push_str("  <Override PartName=\"/ppt/slides/slide");
            entries.push_str(&num.to_string());
            entries.push_str(".xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>
");
        }
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>
  <Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
  <Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>
  <Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
  <Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/>
  <Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/>
  <Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
{}
</Types>"#,
            entries
        )
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_types_contains_slide_overrides() {
        let slides = vec![(1, "image1.png".into())];
        let ct = ContentTypesXml::new(&slides);
        let xml = ct.to_string();
        assert!(xml.contains("slide1.xml"));
        assert!(xml.contains("presentationml.slide+xml"));
        assert!(xml.contains("image/png"));
        assert!(xml.contains(r#"Default Extension="png""#));
    }

    #[test]
    fn test_multiple_slides() {
        let slides = vec![(1, "img1.png".into()), (2, "img2.png".into()), (3, "img3.png".into())];
        let ct = ContentTypesXml::new(&slides);
        let xml = ct.to_string();
        assert!(xml.contains("slide1.xml"));
        assert!(xml.contains("slide2.xml"));
        assert!(xml.contains("slide3.xml"));
    }

    #[test]
    fn test_empty_slides_no_overrides() {
        let ct = ContentTypesXml::new(&[]);
        let xml = ct.to_string();
        assert!(!xml.contains("slide0.xml"));
        assert!(xml.contains("<Types"));
        assert!(xml.contains("</Types>"));
    }

    #[test]
    fn test_png_default_extension() {
        let ct = ContentTypesXml::new(&[]);
        let xml = ct.to_string();
        assert!(xml.contains(r#"Default Extension="png" ContentType="image/png""#));
    }
}

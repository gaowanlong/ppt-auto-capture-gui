//! Generates slide XML and slide relationships XML.

/// EMU constants
const SLIDE_W_4_3: i64 = 9144000; // 10 inches for 4:3
const SLIDE_H_4_3: i64 = 6858000; // 7.5 inches for 4:3
const SLIDE_W_16_9: i64 = 9144000; // 10 inches for 16:9
const SLIDE_H_16_9: i64 = 5143500; // 5.625 inches for 16:9
const EMU_PER_PX: i64 = 12700; // 1 pixel at 72 DPI ≈ 12700 EMU

/// Compute slide dimensions in EMU from a ratio string like "16:9" or "4:3".
pub fn slide_dimensions(ratio: &str) -> (i64, i64) {
    match ratio {
        "4:3" => (SLIDE_W_4_3, SLIDE_H_4_3),
        "3:2" => (SLIDE_W_16_9 * 2 / 3, SLIDE_H_16_9), // approximate
        "16:10" => (SLIDE_W_16_9, SLIDE_H_4_3 * 10 / 15),
        _ => (SLIDE_W_16_9, SLIDE_H_16_9), // default 16:9
    }
}

/// Compute image display position/size in EMU for "fit" mode (maintain aspect ratio, center).
fn compute_image_fit(
    img_w_px: u32,
    img_h_px: u32,
    slide_w: i64,
    slide_h: i64,
) -> (i64, i64, i64, i64) {
    let img_emu_w = (img_w_px as i64) * EMU_PER_PX;
    let img_emu_h = (img_h_px as i64) * EMU_PER_PX;

    // Scale to fit within slide, maintaining aspect ratio
    let scale_x = slide_w as f64 / img_emu_w as f64;
    let scale_y = slide_h as f64 / img_emu_h as f64;
    let scale = scale_x.min(scale_y).min(1.0); // Don't upscale

    let disp_w = (img_emu_w as f64 * scale) as i64;
    let disp_h = (img_emu_h as f64 * scale) as i64;

    // Center on slide
    let off_x = (slide_w - disp_w) / 2;
    let off_y = (slide_h - disp_h) / 2;

    (off_x, off_y, disp_w, disp_h)
}

/// Generates slide XML and rels XML for a given slide number.
pub struct SlideXml;

impl SlideXml {
    /// Returns (slide_xml, rels_xml) pair.
    /// image_w/image_h are the pixel dimensions of the captured screenshot.
    /// fit_mode is "fill" (stretch to fill) or "fit" (proportional, centered).
    /// page_ratio is "16:9", "4:3", etc.
    pub fn render(
        slide_number: u32,
        image_name: &str,
        image_w: u32,
        image_h: u32,
        fit_mode: &str,
        page_ratio: &str,
    ) -> (String, String) {
        let (slide_w, slide_h) = slide_dimensions(page_ratio);
        let (off_x, off_y, disp_w, disp_h) = if fit_mode == "fit" {
            compute_image_fit(image_w, image_h, slide_w, slide_h)
        } else {
            (0i64, 0i64, slide_w, slide_h)
        };

        let slide_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
       xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld name="Slide {}">
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="0" cy="0"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="0" cy="0"/>
        </a:xfrm>
      </p:grpSpPr>
      <p:pic>
        <p:nvPicPr>
          <p:cNvPr id="2" name="{}"/>
          <p:cNvPicPr/>
          <p:nvPr/>
        </p:nvPicPr>
        <p:blipFill>
          <a:blip r:embed="rId2"/>
          <a:stretch>
            <a:fillRect/>
          </a:stretch>
        </p:blipFill>
        <p:spPr>
          <a:xfrm>
            <a:off x="{}" y="{}"/>
            <a:ext cx="{}" cy="{}"/>
          </a:xfrm>
          <a:prstGeom prst="rect">
            <a:avLst/>
          </a:prstGeom>
        </p:spPr>
      </p:pic>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr>
    <a:masterClrMapping/>
  </p:clrMapOvr>
</p:sld>"#,
            slide_number, image_name, off_x, off_y, disp_w, disp_h
        );

        let rels_xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1"
                Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout"
                Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2"
                Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                Target="../media/{}.png"/>
</Relationships>"#,
            image_name
        );

        (slide_xml, rels_xml)
    }
}

/// Presentation XML (lists all slides).
pub struct PresentationXml;

impl PresentationXml {
    pub fn render(slides: &[(u32, String)], page_ratio: &str) -> String {
        let mut sld_ids = String::new();
        for (num, _media) in slides {
            sld_ids.push_str(&format!(
                r#"        <p:sldId id="{}" r:id="rId{}"/>"#,
                255 + num,
                num + 1 // rId1 = master, slides start at rId2
            ));
            sld_ids.push('\n');
        }

        let (sld_w, sld_h) = slide_dimensions(page_ratio);
        let slide_type = if page_ratio == "16:9" {
            r#" type="screen16x9""#
        } else {
            ""
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:sldMasterIdLst>
    <p:sldMasterId id="2147483648" r:id="rId1"/>
  </p:sldMasterIdLst>
  <p:sldIdLst>
{}
  </p:sldIdLst>
  <p:sldSz cx="{}" cy="{}"{}/>
  <p:notesSz cx="6858000" cy="9144000"/>
  <p:defaultTextStyle>
    <a:defPPr>
      <a:defRPr lang="zh-CN"/>
    </a:defPPr>
  </p:defaultTextStyle>
</p:presentation>"#,
            sld_ids, sld_w, sld_h, slide_type
        )
    }
}

/// Presentation relationships XML.
pub struct PresentationRelsXml;

impl PresentationRelsXml {
    pub fn render(slides: &[(u32, String)]) -> String {
        let mut slide_rels = String::new();
        for (num, _media) in slides {
            slide_rels.push_str(&format!(
                r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{}.xml"/>"#,
                num + 1, num  // rId1 = master, slides start at rId2
            ));
            slide_rels.push('\n');
        }

        let next_id = slides.iter().map(|(num, _)| num + 1).max().unwrap_or(1) + 1;
        let support_rels = [
            (
                "presProps",
                "presProps.xml",
            ),
            (
                "viewProps",
                "viewProps.xml",
            ),
            (
                "theme",
                "theme/theme1.xml",
            ),
            (
                "tableStyles",
                "tableStyles.xml",
            ),
        ]
        .iter()
        .enumerate()
        .map(|(offset, (relationship_type, target))| {
            format!(
                r#"  <Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/{}" Target="{}"/>"#,
                next_id + offset as u32,
                relationship_type,
                target
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>
{}
{}
</Relationships>"#,
            slide_rels, support_rels
        )
    }
}

// --- Static XML templates ---

pub const RELS_DOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#;

pub const SLIDE_MASTER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld name="Slide Master">
    <p:bg>
      <p:bgPr>
        <a:solidFill><a:schemeClr val="bg1"/></a:solidFill>
        <a:effectLst/>
      </p:bgPr>
    </p:bg>
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="0" cy="0"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="0" cy="0"/>
        </a:xfrm>
      </p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2"
            accent1="accent1" accent2="accent2" accent3="accent3"
            accent4="accent4" accent5="accent5" accent6="accent6"
            hlink="hlink" folHlink="folHlink"/>
  <p:sldLayoutIdLst>
    <p:sldLayoutId id="2147483649" r:id="rId1"/>
  </p:sldLayoutIdLst>
  <p:txStyles>
    <p:titleStyle>
      <a:defPPr><a:defRPr lang="zh-CN"/></a:defPPr>
    </p:titleStyle>
    <p:bodyStyle>
      <a:defPPr><a:defRPr lang="zh-CN"/></a:defPPr>
    </p:bodyStyle>
    <p:otherStyle>
      <a:defPPr><a:defRPr lang="zh-CN"/></a:defPPr>
    </p:otherStyle>
  </p:txStyles>
</p:sldMaster>"#;

pub const SLIDE_MASTER_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/>
</Relationships>"#;

pub const SLIDE_LAYOUT_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
             xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"
             type="blank" preserve="1">
  <p:cSld name="Blank Layout">
    <p:spTree>
      <p:nvGrpSpPr>
        <p:cNvPr id="1" name=""/>
        <p:cNvGrpSpPr/>
        <p:nvPr/>
      </p:nvGrpSpPr>
      <p:grpSpPr>
        <a:xfrm>
          <a:off x="0" y="0"/>
          <a:ext cx="0" cy="0"/>
          <a:chOff x="0" y="0"/>
          <a:chExt cx="0" cy="0"/>
        </a:xfrm>
      </p:grpSpPr>
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr>
    <a:masterClrMapping/>
  </p:clrMapOvr>
</p:sldLayout>"#;

pub const SLIDE_LAYOUT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/>
</Relationships>"#;

pub const THEME_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Default">
  <a:themeElements>
    <a:clrScheme name="Default">
      <a:dk1><a:srgbClr val="000000"/></a:dk1>
      <a:lt1><a:srgbClr val="FFFFFF"/></a:lt1>
      <a:dk2><a:srgbClr val="44546A"/></a:dk2>
      <a:lt2><a:srgbClr val="E7E6E6"/></a:lt2>
      <a:accent1><a:srgbClr val="4472C4"/></a:accent1>
      <a:accent2><a:srgbClr val="ED7D31"/></a:accent2>
      <a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>
      <a:accent4><a:srgbClr val="FFC000"/></a:accent4>
      <a:accent5><a:srgbClr val="5B9BD5"/></a:accent5>
      <a:accent6><a:srgbClr val="70AD47"/></a:accent6>
      <a:hlink><a:srgbClr val="0563C1"/></a:hlink>
      <a:folHlink><a:srgbClr val="954F72"/></a:folHlink>
    </a:clrScheme>
    <a:fontScheme name="Default">
      <a:majorFont>
        <a:latin typeface="Calibri Light"/>
        <a:ea typeface=""/>
        <a:cs typeface=""/>
      </a:majorFont>
      <a:minorFont>
        <a:latin typeface="Calibri"/>
        <a:ea typeface=""/>
        <a:cs typeface=""/>
      </a:minorFont>
    </a:fontScheme>
    <a:fmtScheme name="Default">
      <a:fillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"><a:tint val="50000"/></a:schemeClr></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"><a:shade val="50000"/></a:schemeClr></a:solidFill>
      </a:fillStyleLst>
      <a:lnStyleLst>
        <a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln>
        <a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln>
        <a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln>
      </a:lnStyleLst>
      <a:effectStyleLst>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
        <a:effectStyle><a:effectLst/></a:effectStyle>
      </a:effectStyleLst>
      <a:bgFillStyleLst>
        <a:solidFill><a:schemeClr val="phClr"/></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"><a:tint val="95000"/></a:schemeClr></a:solidFill>
        <a:solidFill><a:schemeClr val="phClr"><a:shade val="90000"/></a:schemeClr></a:solidFill>
      </a:bgFillStyleLst>
    </a:fmtScheme>
  </a:themeElements>
</a:theme>"#;

pub const PRES_PROPS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:presentationPr xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"/>
"#;

pub const TABLE_STYLES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<a:tblStyleLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" def="{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}"/>"#;

pub const VIEW_PROPS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:viewPr xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:normalViewPr><p:restoredLeft sz="15611"/><p:restoredTop sz="94660"/></p:normalViewPr>
</p:viewPr>"#;

// Dynamic — slide count is filled in add_slide()
pub const DOC_PROPS_APP_XML_TEMPLATE_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"
            xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes">
  <Application>PPT Auto Capture</Application>
  <Slides>0</Slides>
</Properties>"#;

pub const DOC_PROPS_CORE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
                   xmlns:dc="http://purl.org/dc/elements/1.1/"
                   xmlns:dcterms="http://purl.org/dc/terms/"
                   xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:creator>PPT Auto Capture</dc:creator>
  <cp:lastModifiedBy>PPT Auto Capture</cp:lastModifiedBy>
  <dcterms:created xsi:type="dcterms:W3CDTF">2025-01-01T00:00:00Z</dcterms:created>
  <dcterms:modified xsi:type="dcterms:W3CDTF">2025-01-01T00:00:00Z</dcterms:modified>
</cp:coreProperties>"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_dimensions_16_9() {
        let (w, h) = slide_dimensions("16:9");
        assert_eq!(w, 9144000);
        assert_eq!(h, 5143500);
    }

    #[test]
    fn test_slide_dimensions_4_3() {
        let (w, h) = slide_dimensions("4:3");
        assert_eq!(w, 9144000);
        assert_eq!(h, 6858000);
    }

    #[test]
    fn test_slide_dimensions_default() {
        let (w, h) = slide_dimensions("unknown");
        assert_eq!(w, 9144000);
        assert_eq!(h, 5143500);
    }

    #[test]
    fn test_compute_image_fit_fits_width() {
        // 1x1 pixel in a 12700x25400 EMU slide → image exactly fits width, centered vertically
        let (ox, oy, dw, dh) = compute_image_fit(1, 2, 12700, 25400);
        assert_eq!(dw, 12700, "width fills slide");
        assert_eq!(dh, 25400, "height fills slide (2x12700)");
        assert_eq!(ox, 0);
        assert_eq!(oy, 0);
    }

    #[test]
    fn test_slide_xml_contains_image() {
        let (xml, rels) = SlideXml::render(1, "image1", 1920, 1080, "fit", "16:9");
        assert!(xml.contains("slide1") || xml.contains("Slide 1"));
        assert!(xml.contains("image1"));
        assert!(xml.contains("p:pic"));
        assert!(rels.contains("rId2"));
        assert!(rels.contains("../media/image1.png"));
    }

    #[test]
    fn test_presentation_xml_contains_slides() {
        let slides = vec![(1, "image1.png".into()), (2, "image2.png".into())];
        let xml = PresentationXml::render(&slides, "16:9");
        assert!(xml.contains("sldId"));
        assert!(xml.contains("rId2")); // slide 1 (rId1 is master)
        assert!(xml.contains("rId3")); // slide 2
        assert!(xml.contains("rId1")); // master reference
        assert!(xml.contains(r#"cx="9144000""#));
        assert!(xml.contains(r#"cy="5143500""#));
    }

    #[test]
    fn test_presentation_rels_contains_slides() {
        let slides = vec![(1, "image1.png".into())];
        let rels = PresentationRelsXml::render(&slides);
        assert!(rels.contains("rId1")); // master
        assert!(rels.contains("rId2")); // slide 1 (offset)
        assert!(rels.contains("slides/slide1.xml"));
    }

    #[test]
    fn presentation_relationships_include_support_parts() {
        let rels = PresentationRelsXml::render(&[(1, "image1.png".into())]);
        for (relationship_type, target) in [
            ("presProps", "presProps.xml"),
            ("viewProps", "viewProps.xml"),
            ("theme", "theme/theme1.xml"),
            ("tableStyles", "tableStyles.xml"),
        ] {
            assert!(
                rels.contains(&format!(
                    "/relationships/{relationship_type}\" Target=\"{target}\""
                )),
                "missing {relationship_type} relationship to {target}"
            );
        }
    }

    #[test]
    fn presentation_relationship_ids_are_unique_for_supported_slide_counts() {
        for slide_count in [0, 1, 3, 100] {
            let slides = (1..=slide_count)
                .map(|number| (number, format!("image{number}.png")))
                .collect::<Vec<_>>();
            let rels = PresentationRelsXml::render(&slides);
            let ids = rels
                .split(" Id=\"")
                .skip(1)
                .map(|tail| tail.split('"').next().unwrap())
                .collect::<Vec<_>>();
            let unique = ids
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();

            assert_eq!(
                ids.len(),
                slide_count as usize + 5,
                "{slide_count} slides require a master, every slide, and four support relationships"
            );
            assert_eq!(
                unique.len(),
                ids.len(),
                "relationship IDs must be unique for {slide_count} slides"
            );
        }
    }

    #[test]
    fn view_properties_use_size_attributes() {
        assert!(VIEW_PROPS_XML.contains("<p:restoredLeft sz=\"15611\"/>"));
        assert!(VIEW_PROPS_XML.contains("<p:restoredTop sz=\"94660\"/>"));
        assert!(!VIEW_PROPS_XML.contains("<p:restoredLeft cx="));
        assert!(!VIEW_PROPS_XML.contains("<p:restoredTop cy="));
    }

    #[test]
    fn theme_has_complete_font_collections_and_style_matrix() {
        for font_name in ["majorFont", "minorFont"] {
            let start = THEME_XML
                .find(&format!("<a:{font_name}>"))
                .expect("font collection must exist");
            let end = THEME_XML[start..]
                .find(&format!("</a:{font_name}>"))
                .expect("font collection must close")
                + start;
            let font = &THEME_XML[start..end];
            for required in ["<a:latin", "<a:ea", "<a:cs"] {
                assert!(
                    font.contains(required),
                    "{font_name} must contain {required}"
                );
            }
        }

        for (list_name, child_name) in [
            ("fillStyleLst", "solidFill"),
            ("lnStyleLst", "ln"),
            ("effectStyleLst", "effectStyle"),
            ("bgFillStyleLst", "solidFill"),
        ] {
            let start = THEME_XML
                .find(&format!("<a:{list_name}>"))
                .expect("style list must exist");
            let end = THEME_XML[start..]
                .find(&format!("</a:{list_name}>"))
                .expect("style list must close")
                + start;
            let list = &THEME_XML[start..end];
            assert!(
                list.matches(&format!("<a:{child_name}")).count() >= 3,
                "{list_name} must contain at least three {child_name} entries"
            );
        }
    }

    #[test]
    fn presentation_and_master_have_powerpoint_compatible_defaults() {
        let presentation = PresentationXml::render(&[(1, "image1.png".into())], "16:9");
        assert!(presentation.contains(r#"<p:sldSz cx="9144000" cy="5143500" type="screen16x9"/>"#));
        assert!(presentation.contains("<p:defaultTextStyle>"));
        assert!(SLIDE_MASTER_XML.contains("<p:bg>"));
        for style in ["titleStyle", "bodyStyle", "otherStyle"] {
            assert!(
                SLIDE_MASTER_XML.contains(&format!("<p:{style}>")),
                "slide master must contain {style}"
            );
        }
    }

    #[test]
    fn test_slide_rel_contains_image() {
        let (_, rels) = SlideXml::render(1, "image1", 640, 480, "fill", "16:9");
        assert!(rels.contains("../media/image1.png"));
    }
}

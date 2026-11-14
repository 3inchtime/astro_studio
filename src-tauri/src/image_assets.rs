//! Decode generated assets without changing their original SVG/GIF bytes.
use crate::error::AppError;
use image::{DynamicImage, ImageReader};
use std::io::Cursor;

const MAX_SVG_BYTES: usize = 4 * 1024 * 1024;
const MAX_SVG_NODES: u32 = 20_000;
const MAX_DIMENSION: u32 = 8192;
const MAX_PIXELS: u64 = 16_777_216;
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

fn invalid(message: impl Into<String>) -> AppError {
    AppError::Validation {
        message: message.into(),
    }
}

fn check_dimensions(width: u32, height: u32) -> Result<(), AppError> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(invalid(
            "Image dimensions exceed the supported preview limit",
        ));
    }
    Ok(())
}

// Reject CSS escapes and directives so URL validation cannot be bypassed with
// escaped identifiers or imports. Only local paint-server references are allowed.
fn check_local_references(value: &str) -> Result<(), AppError> {
    let lower = value.to_ascii_lowercase();
    if lower.contains(['\\', '@']) || lower.contains("/*") {
        return Err(invalid("SVG contains unsupported CSS syntax"));
    }
    let mut remainder = lower.as_str();
    while let Some(start) = remainder.find("url") {
        remainder = remainder[start + 3..].trim_start();
        if let Some(url) = remainder.strip_prefix('(') {
            let end = url.find(')').ok_or_else(|| invalid("Invalid SVG URL"))?;
            let target = url[..end].trim().trim_matches(['\'', '"']);
            if !target.starts_with('#') || target.len() < 2 {
                return Err(invalid("SVG external resources are not supported"));
            }
            remainder = &url[end + 1..];
        }
    }
    Ok(())
}

fn validated_svg(data: &[u8]) -> Result<&str, AppError> {
    if data.len() > MAX_SVG_BYTES {
        return Err(invalid("SVG exceeds the 4 MiB document limit"));
    }
    let source = std::str::from_utf8(data).map_err(|_| invalid("SVG must be UTF-8 XML"))?;
    let document = roxmltree::Document::parse_with_options(
        source,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_SVG_NODES,
            ..Default::default()
        },
    )
    .map_err(|error| invalid(format!("Invalid SVG: {error}")))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" || root.tag_name().namespace() != Some(SVG_NAMESPACE) {
        return Err(invalid("Expected an SVG document"));
    }
    for node in document.descendants() {
        if node.is_pi() {
            return Err(invalid("SVG processing instructions are not supported"));
        }
        if !node.is_element() {
            continue;
        }
        if node.ancestors().take(66).count() > 65 {
            return Err(invalid("SVG nesting exceeds the supported limit"));
        }
        // Deliberately exclude scripts, foreignObject, animation, image, filters,
        // use (tree expansion), and patterns (unbounded tile allocations).
        if node.tag_name().namespace() != Some(SVG_NAMESPACE)
            || !matches!(
                node.tag_name().name(),
                "svg"
                    | "g"
                    | "defs"
                    | "path"
                    | "rect"
                    | "circle"
                    | "ellipse"
                    | "line"
                    | "polyline"
                    | "polygon"
                    | "linearGradient"
                    | "radialGradient"
                    | "stop"
                    | "clipPath"
                    | "mask"
                    | "text"
                    | "tspan"
                    | "textPath"
                    | "title"
                    | "desc"
                    | "style"
            )
        {
            return Err(invalid(format!(
                "Unsupported SVG element: {}",
                node.tag_name().name()
            )));
        }
        for attr in node.attributes() {
            let name = attr.name().to_ascii_lowercase();
            if name.starts_with("on") || name == "base" || name == "src" {
                return Err(invalid("SVG active content is not supported"));
            }
            if let Some(namespace) = attr.namespace() {
                if !(namespace == "http://www.w3.org/1999/xlink" && name == "href"
                    || namespace == "http://www.w3.org/XML/1998/namespace"
                        && matches!(name.as_str(), "space" | "lang"))
                {
                    return Err(invalid("Unsupported SVG attribute namespace"));
                }
            }
            if name == "href" && !attr.value().trim().starts_with('#') {
                return Err(invalid("SVG external resources are not supported"));
            }
            check_local_references(attr.value())?;
        }
        if node.tag_name().name() == "style" {
            let css: String = node
                .descendants()
                .filter(|child| child.is_text())
                .map(|child| child.text().unwrap_or_default())
                .collect();
            check_local_references(&css)?;
        }
    }
    Ok(source)
}

// Bound intermediate compositing surfaces as well as the final canvas. Counting
// all surfaces is conservative (some are sequential), but prevents a deep stack
// of opacity groups, masks, or clips from multiplying the raster memory budget.
fn check_svg_raster_budget(tree: &resvg::usvg::Tree) -> Result<(), AppError> {
    fn add(used: &mut u64, pixels: u64) -> Result<(), AppError> {
        *used = used.saturating_add(pixels);
        if *used > 64 * 1024 * 1024 {
            return Err(invalid("SVG compositing exceeds the 256 MiB raster budget"));
        }
        Ok(())
    }
    fn visit(
        group: &resvg::usvg::Group,
        used: &mut u64,
        visits: &mut u32,
        depth: u32,
    ) -> Result<(), AppError> {
        *visits += 1;
        if *visits > MAX_SVG_NODES || depth > 64 {
            return Err(invalid(
                "SVG referenced content exceeds the complexity limit",
            ));
        }
        if group.should_isolate() {
            let bounds = group.abs_layer_bounding_box();
            let width = (bounds.width().ceil() as u64).saturating_add(4);
            let height = (bounds.height().ceil() as u64).saturating_add(4);
            let pixels = width.saturating_mul(height);
            add(used, pixels)?;
            let mut clip = group.clip_path();
            while let Some(current) = clip {
                add(used, pixels.saturating_mul(2))?;
                visit(current.root(), used, visits, depth + 1)?;
                clip = current.clip_path();
            }
            let mut mask = group.mask();
            while let Some(current) = mask {
                add(used, pixels.saturating_mul(2))?;
                visit(current.root(), used, visits, depth + 1)?;
                mask = current.mask();
            }
        }
        for child in group.children() {
            match child {
                resvg::usvg::Node::Group(child) => visit(child, used, visits, depth + 1)?,
                resvg::usvg::Node::Text(text) => visit(text.flattened(), used, visits, depth + 1)?,
                _ => {}
            }
        }
        Ok(())
    }
    let size = tree.size().to_int_size();
    let mut used = u64::from(size.width()) * u64::from(size.height()) * 2;
    visit(tree.root(), &mut used, &mut 0, 0)
}

pub(crate) fn is_safe_svg(data: &[u8]) -> bool {
    validated_svg(data).is_ok()
}

pub(crate) fn decode_image_for_preview(data: &[u8]) -> Result<DynamicImage, AppError> {
    if image::guess_format(data).is_ok() {
        let mut reader = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .map_err(|error| invalid(format!("Decode image failed: {error}")))?;
        let (width, height) = ImageReader::new(Cursor::new(data))
            .with_guessed_format()
            .map_err(|error| invalid(format!("Decode image failed: {error}")))?
            .into_dimensions()
            .map_err(|error| invalid(format!("Decode image failed: {error}")))?;
        check_dimensions(width, height)?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        limits.max_alloc = Some(256 * 1024 * 1024);
        reader.limits(limits);
        return reader
            .decode()
            .map_err(|error| invalid(format!("Decode image failed: {error}")));
    }
    let source = validated_svg(data)?;
    static FONTS: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> =
        std::sync::OnceLock::new();
    let options = resvg::usvg::Options {
        // Even if validation evolves, never resolve document-supplied resources.
        image_href_resolver: resvg::usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        fontdb: FONTS
            .get_or_init(|| {
                let mut fonts = resvg::usvg::fontdb::Database::new();
                fonts.load_system_fonts();
                std::sync::Arc::new(fonts)
            })
            .clone(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(source, &options)
        .map_err(|error| invalid(format!("Decode SVG failed: {error}")))?;
    let size = tree.size().to_int_size();
    check_dimensions(size.width(), size.height())?;
    check_svg_raster_budget(&tree)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width(), size.height())
        .ok_or_else(|| invalid("Unable to allocate SVG preview"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    // tiny-skia stores premultiplied RGBA; image/clipboard require straight RGBA.
    let mut rgba = pixmap.data().to_vec();
    for pixel in rgba.as_chunks_mut::<4>().0 {
        let alpha = u32::from(pixel[3]);
        if alpha > 0 && alpha < 255 {
            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    let image = image::RgbaImage::from_raw(size.width(), size.height(), rgba)
        .ok_or_else(|| invalid("Invalid SVG preview buffer"))?;
    Ok(DynamicImage::ImageRgba8(image))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    fn svg(content: &str) -> Vec<u8> {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="16">{content}</svg>"#)
            .into_bytes()
    }

    #[test]
    fn preview_renders_vectors_and_preserves_straight_alpha() {
        let preview = decode_image_for_preview(&svg(
            r#"<rect width="32" height="16" fill="red" opacity="0.5"/>"#,
        ))
        .unwrap();
        assert_eq!(preview.dimensions(), (32, 16));
        let pixel = preview.to_rgba8().get_pixel(4, 4).0;
        assert_eq!(pixel[0], 255);
        assert!(pixel[3] == 127 || pixel[3] == 128);
    }

    #[test]
    fn rejects_active_external_and_expanding_svg_documents() {
        for content in [
            r#"<script>alert(1)</script>"#,
            r#"<rect onload="alert(1)"/>"#,
            r#"<foreignObject/>"#,
            r#"<animate attributeName="x"/>"#,
            r#"<image href="data:image/png;base64,iVBORw0KGgo="/>"#,
            r#"<image href="file:///etc/passwd"/>"#,
            r#"<linearGradient href="https://example.com/g.svg#a"/>"#,
            r#"<rect fill="url(https://example.com/g.svg#a)"/>"#,
            r#"<style>@import 'https://example.com/a.css';</style>"#,
            r#"<style>rect { fill: u\72l(https://example.com/a); }</style>"#,
            r##"<use href="#a"/>"##,
            r#"<filter id="blur"/>"#,
            r#"<pattern width="100000000" height="100000000"/>"#,
            r#"<?xml-stylesheet href="https://example.com/a.css"?>"#,
        ] {
            assert!(!is_safe_svg(&svg(content)), "accepted {content}");
            assert!(
                decode_image_for_preview(&svg(content)).is_err(),
                "decoded {content}"
            );
        }
        let dtd = b"<!DOCTYPE svg [<!ENTITY text 'unsafe'>]><svg xmlns='http://www.w3.org/2000/svg'>&text;</svg>";
        assert!(decode_image_for_preview(dtd).is_err());
        assert!(!is_safe_svg(
            b"<svg xmlns='http://www.w3.org/2000/svg'>\xff</svg>"
        ));
    }

    #[test]
    fn rejects_oversized_and_deep_svg_before_rasterization() {
        for (width, height) in [(9000, 1), (8192, 8192), (0, 16)] {
            let source = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"/>"#
            );
            assert!(decode_image_for_preview(source.as_bytes()).is_err());
        }
        assert!(!is_safe_svg(&svg(&" ".repeat(MAX_SVG_BYTES))));
        assert!(!is_safe_svg(&svg(&format!(
            "{}{}",
            "<g>".repeat(65),
            "</g>".repeat(65)
        ))));
        assert!(!is_safe_svg(
            &svg(&"<path/>".repeat(MAX_SVG_NODES as usize))
        ));
    }

    #[test]
    fn allows_local_gradient_references_and_utf8_labels() {
        let source = svg(
            r##"<title>图形</title><defs><linearGradient id="a"><stop stop-color="red"/></linearGradient></defs><rect width="32" height="16" fill="url(#a)"/>"##,
        );
        assert!(is_safe_svg(&source));
        assert_eq!(
            decode_image_for_preview(&source)
                .unwrap()
                .to_rgba8()
                .get_pixel(4, 4)
                .0,
            [255, 0, 0, 255]
        );
    }

    #[test]
    fn rejects_excessive_compositing_buffers_before_rendering() {
        let layers = format!(
            "{}<rect width='4096' height='4096' fill='red'/>{}",
            "<g opacity='0.5'>".repeat(8),
            "</g>".repeat(8)
        );
        let source =
            format!("<svg xmlns='{SVG_NAMESPACE}' width='4096' height='4096'>{layers}</svg>");
        // Validate the renderer tree without allocating its large pixel buffers.
        let tree = resvg::usvg::Tree::from_str(&source, &resvg::usvg::Options::default()).unwrap();
        assert!(check_svg_raster_budget(&tree).is_err());
    }

    #[test]
    fn rejects_css_references_split_by_xml_comments() {
        let source =
            svg(r#"<style>rect { fill: u<!-- split -->rl(https://example.com/a); }</style>"#);
        assert!(!is_safe_svg(&source));
    }

    #[test]
    fn enormous_geometry_never_panics_during_budget_check() {
        let source = svg("<g opacity='0.5'><rect width='1e30' height='1e30'/></g>");
        assert!(decode_image_for_preview(&source).is_err());
    }
}

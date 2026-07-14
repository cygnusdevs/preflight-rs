// SPDX-License-Identifier: AGPL-3.0-or-later

use mupdf::{
    pdf::{PdfDocument, Permission},
    Document, DocumentWriter, Matrix, Rect, TextBlockContent, TextPageFlags,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const PT_TO_MM: f64 = 25.4 / 72.0;
const MM_PER_INCH: f64 = 25.4;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PageSizeMm {
    pub w_mm: f64,
    pub h_mm: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectMm {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl RectMm {
    pub fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn width(self) -> f64 {
        (self.x1 - self.x0).max(0.0)
    }

    pub fn height(self) -> f64 {
        (self.y1 - self.y0).max(0.0)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageContentFact {
    pub page: u32,
    pub text_chars: usize,
    pub image_count: usize,
    pub drawing_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageFact {
    pub page: u32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub placed: PageSizeMm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageFact {
    pub page: u32,
    pub size: PageSizeMm,
    pub content_bbox: Option<RectMm>,
    pub content: PageContentFact,
    pub images: Vec<ImageFact>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PdfInspection {
    pub readable: bool,
    pub encrypted: bool,
    pub printing_disallowed: bool,
    pub page_count: Option<u32>,
    pub pages: Vec<PageFact>,
}

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("mupdf")]
    Mupdf,
}

pub fn has_pdf_magic(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%PDF")
}

pub fn pdf_version(bytes: &[u8]) -> Option<String> {
    let first_line = bytes
        .split(|byte| *byte == b'\n' || *byte == b'\r')
        .next()?;
    let text = std::str::from_utf8(first_line).ok()?;
    text.strip_prefix("%PDF-").map(str::to_owned)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    format!("{digest:x}")
}

pub fn is_a4_size_mm(w_mm: f64, h_mm: f64) -> bool {
    fn near(value: f64, target: f64) -> bool {
        (value - target).abs() <= 2.0
    }

    (near(w_mm, 210.0) && near(h_mm, 297.0)) || (near(w_mm, 297.0) && near(h_mm, 210.0))
}

pub fn is_tight_to_edge(page: RectMm, content: RectMm, threshold_mm: f64) -> bool {
    (content.x0 - page.x0) < threshold_mm
        || (content.y0 - page.y0) < threshold_mm
        || (page.x1 - content.x1) < threshold_mm
        || (page.y1 - content.y1) < threshold_mm
}

pub fn is_blank_page(content: &PageContentFact) -> bool {
    content.text_chars == 0 && content.image_count == 0 && content.drawing_count == 0
}

pub fn image_dpi(image: &ImageFact) -> f64 {
    let width_inches = image.placed.w_mm / MM_PER_INCH;
    let height_inches = image.placed.h_mm / MM_PER_INCH;
    if width_inches <= 0.0 || height_inches <= 0.0 {
        return 0.0;
    }
    let x_dpi = f64::from(image.pixel_width) / width_inches;
    let y_dpi = f64::from(image.pixel_height) / height_inches;
    x_dpi.min(y_dpi)
}

pub fn fit_pdf_to_a4(bytes: &[u8], margin_mm: f64) -> Result<Vec<u8>, PdfError> {
    const A4_WIDTH_POINTS: f32 = 210.0 * 72.0 / 25.4;
    const A4_HEIGHT_POINTS: f32 = 297.0 * 72.0 / 25.4;

    let document = Document::from_bytes(bytes, "pdf").map_err(|_| PdfError::Mupdf)?;
    let directory = tempfile::tempdir().map_err(|_| PdfError::Mupdf)?;
    let output = directory.path().join("fitted.pdf");
    let output_path = output.to_str().ok_or(PdfError::Mupdf)?;
    let a4 = Rect::new(0.0, 0.0, A4_WIDTH_POINTS, A4_HEIGHT_POINTS);

    {
        let mut writer =
            DocumentWriter::new(output_path, "pdf", "").map_err(|_| PdfError::Mupdf)?;
        let page_count = document.page_count().map_err(|_| PdfError::Mupdf)?;
        for page_number in 0..page_count {
            let page = document
                .load_page(page_number)
                .map_err(|_| PdfError::Mupdf)?;
            let bounds = page.bounds().map_err(|_| PdfError::Mupdf)?;
            let matrix = fit_matrix(bounds, margin_mm as f32).ok_or(PdfError::Mupdf)?;
            let device = writer.begin_page(a4).map_err(|_| PdfError::Mupdf)?;
            page.run(&device, &matrix).map_err(|_| PdfError::Mupdf)?;
            writer.end_page(device).map_err(|_| PdfError::Mupdf)?;
        }
    }

    std::fs::read(output).map_err(|_| PdfError::Mupdf)
}

fn fit_matrix(source: Rect, margin_mm: f32) -> Option<Matrix> {
    const A4_WIDTH_POINTS: f32 = 210.0 * 72.0 / 25.4;
    const A4_HEIGHT_POINTS: f32 = 297.0 * 72.0 / 25.4;

    let margin = margin_mm * 72.0 / 25.4;
    let available_width = A4_WIDTH_POINTS - (margin * 2.0);
    let available_height = A4_HEIGHT_POINTS - (margin * 2.0);
    if !margin.is_finite()
        || margin < 0.0
        || available_width <= 0.0
        || available_height <= 0.0
        || source.is_empty()
    {
        return None;
    }

    let scale = (available_width / source.width()).min(available_height / source.height());
    let x = margin + ((available_width - source.width() * scale) / 2.0) - source.x0 * scale;
    let y = margin + ((available_height - source.height() * scale) / 2.0) - source.y0 * scale;

    Some(Matrix::new(scale, 0.0, 0.0, scale, x, y))
}

pub fn inspect_pdf(bytes: &[u8]) -> Result<PdfInspection, PdfError> {
    let doc = Document::from_bytes(bytes, "pdf").map_err(|_| PdfError::Mupdf)?;
    let encrypted = doc.needs_password().map_err(|_| PdfError::Mupdf)?;
    let pdf_doc = PdfDocument::from_bytes(bytes).ok();
    let printing_disallowed = pdf_doc
        .as_ref()
        .map(|pdf| !pdf.permissions().contains(Permission::PRINT))
        .unwrap_or(false);

    if encrypted {
        return Ok(PdfInspection {
            readable: true,
            encrypted: true,
            printing_disallowed,
            page_count: None,
            pages: Vec::new(),
        });
    }

    let page_count = doc.page_count().map_err(|_| PdfError::Mupdf)?.max(0) as u32;
    let mut pages = Vec::new();

    for index in 0..page_count {
        let page_no = index as i32;
        let page = doc.load_page(page_no).map_err(|_| PdfError::Mupdf)?;
        let bounds = page.bounds().map_err(|_| PdfError::Mupdf)?;
        let size = PageSizeMm {
            w_mm: f64::from(bounds.width()) * PT_TO_MM,
            h_mm: f64::from(bounds.height()) * PT_TO_MM,
        };

        let text_page = page
            .to_text_page(TextPageFlags::empty())
            .map_err(|_| PdfError::Mupdf)?;
        let text = text_page.to_text().map_err(|_| PdfError::Mupdf)?;
        let structured = text_page.structured();
        let words = text_page.words();
        let mut content_bbox: Option<Rect> = None;

        for word in words {
            union_rect(&mut content_bbox, word.bounds);
        }

        let mut image_blocks = Vec::new();
        for block in structured.blocks {
            if matches!(block.content, TextBlockContent::Image { .. }) {
                union_rect(&mut content_bbox, block.bounds);
                image_blocks.push(block.bounds);
            }
        }

        let drawing_count = if let Ok(drawings) = page.drawings() {
            let count = drawings.len();
            for drawing in drawings {
                union_rect(&mut content_bbox, drawing.rect);
            }
            count
        } else {
            0
        };

        let page_images = pdf_doc
            .as_ref()
            .and_then(|pdf| pdf.load_pdf_page(page_no).ok())
            .and_then(|pdf_page| pdf_page.images().ok())
            .unwrap_or_default();

        let images = image_facts(index + 1, &page_images, &image_blocks);

        pages.push(PageFact {
            page: index + 1,
            size,
            content_bbox: content_bbox.map(rect_mm),
            content: PageContentFact {
                page: index + 1,
                text_chars: text.trim().chars().count(),
                image_count: page_images.len().max(image_blocks.len()),
                drawing_count,
            },
            images,
        });
    }

    Ok(PdfInspection {
        readable: true,
        encrypted,
        printing_disallowed,
        page_count: Some(page_count),
        pages,
    })
}

fn image_facts(
    page: u32,
    images: &[mupdf::pdf::PageImageInfo],
    placements: &[Rect],
) -> Vec<ImageFact> {
    // ponytail: resources and placements are separate; omit ambiguous multi-image DPI until
    // MuPDF device callbacks can pair them reliably.
    if images.len() != 1 || placements.len() != 1 {
        return Vec::new();
    }

    vec![ImageFact {
        page,
        pixel_width: images[0].width,
        pixel_height: images[0].height,
        placed: rect_size_mm(placements[0]),
    }]
}

pub fn inspect_pdf_metadata(bytes: &[u8]) -> Result<PdfInspection, PdfError> {
    let document = Document::from_bytes(bytes, "pdf").map_err(|_| PdfError::Mupdf)?;
    let encrypted = document.needs_password().map_err(|_| PdfError::Mupdf)?;
    let pdf_document = PdfDocument::from_bytes(bytes).ok();
    let printing_disallowed = pdf_document
        .as_ref()
        .map(|pdf| !pdf.permissions().contains(Permission::PRINT))
        .unwrap_or(false);
    let page_count = if encrypted {
        None
    } else {
        Some(document.page_count().map_err(|_| PdfError::Mupdf)?.max(0) as u32)
    };

    Ok(PdfInspection {
        readable: true,
        encrypted,
        printing_disallowed,
        page_count,
        pages: Vec::new(),
    })
}

fn union_rect(target: &mut Option<Rect>, rect: Rect) {
    if rect.is_empty() || !rect.is_valid() {
        return;
    }
    *target = Some(match *target {
        Some(existing) => existing.r#union(&rect),
        None => rect,
    });
}

fn rect_mm(rect: Rect) -> RectMm {
    RectMm::new(
        f64::from(rect.x0) * PT_TO_MM,
        f64::from(rect.y0) * PT_TO_MM,
        f64::from(rect.x1) * PT_TO_MM,
        f64::from(rect.y1) * PT_TO_MM,
    )
}

fn rect_size_mm(rect: Rect) -> PageSizeMm {
    PageSizeMm {
        w_mm: f64::from(rect.width()) * PT_TO_MM,
        h_mm: f64::from(rect.height()) * PT_TO_MM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_matrix_centres_content_inside_a4_margin() {
        let source = Rect::new(0.0, 0.0, 500.0, 500.0);
        let fitted = source.transform(&fit_matrix(source, 5.0).unwrap());
        let margin_points = 5.0 * 72.0 / 25.4;
        let a4 = Rect::new(0.0, 0.0, 210.0 * 72.0 / 25.4, 297.0 * 72.0 / 25.4);

        assert!((fitted.x0 - margin_points).abs() < 0.01);
        assert!((fitted.x1 - (a4.x1 - margin_points)).abs() < 0.01);
        assert!(fitted.y0 >= margin_points);
        assert!(fitted.y1 <= a4.y1 - margin_points);
        assert!((fitted.width() - fitted.height()).abs() < 0.01);
    }

    #[test]
    fn ambiguous_image_placements_are_not_guessed() {
        let images = vec![extracted_image(1), extracted_image(2)];
        let placements = vec![
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(100.0, 100.0, 200.0, 200.0),
        ];

        assert!(image_facts(1, &images, &placements,).is_empty());
    }

    fn extracted_image(xref: i32) -> mupdf::pdf::PageImageInfo {
        mupdf::pdf::PageImageInfo {
            name: format!("Im{xref}"),
            xref,
            width: 300,
            height: 300,
            bits_per_component: Some(8),
            color_space: Some("DeviceRGB".to_owned()),
            filter: None,
        }
    }
}

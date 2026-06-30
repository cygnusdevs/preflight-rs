// SPDX-License-Identifier: AGPL-3.0-or-later

use mupdf::{
    pdf::{PdfDocument, Permission},
    Document, Rect, TextBlockContent, TextPageFlags,
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
    pub restrictive_permissions: bool,
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
    content.text_chars == 0 && content.image_count == 0
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

pub fn inspect_pdf(bytes: &[u8]) -> Result<PdfInspection, PdfError> {
    let doc = Document::from_bytes(bytes, "pdf").map_err(|_| PdfError::Mupdf)?;
    let encrypted = doc.needs_password().map_err(|_| PdfError::Mupdf)?;
    let pdf_doc = PdfDocument::from_bytes(bytes).ok();
    let restrictive_permissions = pdf_doc
        .as_ref()
        .map(|pdf| pdf.permissions().bits() != Permission::all().bits())
        .unwrap_or(false);

    if encrypted {
        return Ok(PdfInspection {
            readable: true,
            encrypted: true,
            restrictive_permissions,
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

        if let Ok(drawings) = page.drawings() {
            for drawing in drawings {
                union_rect(&mut content_bbox, drawing.rect);
            }
        }

        let page_images = pdf_doc
            .as_ref()
            .and_then(|pdf| pdf.load_pdf_page(page_no).ok())
            .and_then(|pdf_page| pdf_page.images().ok())
            .unwrap_or_default();

        let images = page_images
            .iter()
            .enumerate()
            .map(|(idx, image)| {
                let placed = image_blocks
                    .get(idx)
                    .copied()
                    .map(rect_size_mm)
                    .unwrap_or(size);
                ImageFact {
                    page: index + 1,
                    pixel_width: image.width,
                    pixel_height: image.height,
                    placed,
                }
            })
            .collect::<Vec<_>>();

        pages.push(PageFact {
            page: index + 1,
            size,
            content_bbox: content_bbox.map(rect_mm),
            content: PageContentFact {
                page: index + 1,
                text_chars: text.trim().chars().count(),
                image_count: page_images.len().max(image_blocks.len()),
            },
            images,
        });
    }

    Ok(PdfInspection {
        readable: true,
        encrypted,
        restrictive_permissions,
        page_count: Some(page_count),
        pages,
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

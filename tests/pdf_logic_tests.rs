// SPDX-License-Identifier: AGPL-3.0-or-later

use preflight_rs::pdf::{
    fit_pdf_to_a4, image_dpi, inspect_pdf, is_a4_size_mm, is_blank_page, is_tight_to_edge,
    ImageFact, PageContentFact, PageSizeMm, RectMm,
};

const NORMAL_PDF: &[u8] = include_bytes!("fixtures/normal_text.pdf");

#[test]
fn a4_tolerance_accepts_portrait_and_landscape_with_two_mm_slack() {
    assert!(is_a4_size_mm(209.1, 297.9));
    assert!(is_a4_size_mm(297.9, 209.1));
    assert!(!is_a4_size_mm(220.0, 297.0));
}

#[test]
fn margin_logic_flags_content_inside_threshold_from_any_edge() {
    let page = RectMm::new(0.0, 0.0, 210.0, 297.0);
    let tight = RectMm::new(3.0, 20.0, 100.0, 120.0);
    let clear = RectMm::new(8.0, 8.0, 202.0, 289.0);

    assert!(is_tight_to_edge(page, tight, 5.0));
    assert!(!is_tight_to_edge(page, clear, 5.0));
}

#[test]
fn blank_page_requires_no_text_images_or_vector_drawings() {
    assert!(is_blank_page(&PageContentFact {
        page: 1,
        text_chars: 0,
        image_count: 0,
        drawing_count: 0,
    }));
    assert!(!is_blank_page(&PageContentFact {
        page: 1,
        text_chars: 1,
        image_count: 0,
        drawing_count: 0,
    }));
    assert!(!is_blank_page(&PageContentFact {
        page: 1,
        text_chars: 0,
        image_count: 1,
        drawing_count: 0,
    }));
    assert!(!is_blank_page(&PageContentFact {
        page: 1,
        text_chars: 0,
        image_count: 0,
        drawing_count: 1,
    }));
}

#[test]
fn image_dpi_uses_placed_size_in_inches() {
    let image = ImageFact {
        page: 1,
        pixel_width: 300,
        pixel_height: 300,
        placed: PageSizeMm {
            w_mm: 50.8,
            h_mm: 25.4,
        },
    };

    assert_eq!(image_dpi(&image).round() as u32, 150);
}

#[test]
fn fitting_pdf_emits_a4_pages_with_preserved_content() {
    let output = fit_pdf_to_a4(NORMAL_PDF, 5.0).unwrap();
    let inspection = inspect_pdf(&output).unwrap();

    assert!(output.starts_with(b"%PDF"));
    assert_ne!(output, NORMAL_PDF);
    assert_eq!(inspection.pages.len(), 1);
    assert!(is_a4_size_mm(
        inspection.pages[0].size.w_mm,
        inspection.pages[0].size.h_mm
    ));
    assert!(inspection.pages[0].content.text_chars > 0);
    assert!(!is_tight_to_edge(
        RectMm::new(0.0, 0.0, 210.0, 297.0),
        inspection.pages[0].content_bbox.unwrap(),
        5.0,
    ));
}

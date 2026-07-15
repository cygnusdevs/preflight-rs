// SPDX-License-Identifier: AGPL-3.0-or-later

use mupdf::{
    pdf::{PdfDocument, PdfObject},
    Rect,
};
use preflight_rs::pdf::{
    fit_pdf_to_a4, image_dpi, inspect_pdf, is_a4_size_mm, is_blank_page, is_tight_to_edge,
    ImageFact, PageContentFact, PageSizeMm, RectMm,
};

const NORMAL_PDF: &[u8] = include_bytes!("fixtures/normal_text.pdf");
const IMAGE_PDF: &[u8] = include_bytes!("fixtures/low_res_image.pdf");

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

#[test]
fn fitting_pdf_places_the_source_page_as_one_form() {
    let output = fit_pdf_to_a4(NORMAL_PDF, 5.0).unwrap();
    let document = PdfDocument::from_bytes(&output).unwrap();
    let page = document.load_pdf_page(0).unwrap();
    let resources = page.object().get_dict("Resources").unwrap().unwrap();
    let xobjects = resources.get_dict("XObject").unwrap().unwrap();
    let fitted_page = xobjects.get_dict("FittedPage").unwrap().unwrap();
    let subtype = fitted_page.get_dict("Subtype").unwrap().unwrap();

    assert_eq!(subtype.as_name().unwrap(), b"Form");
}

#[test]
fn fitting_pdf_bakes_annotations_into_the_preserved_page() {
    let document = PdfDocument::from_bytes(NORMAL_PDF).unwrap();
    let mut source_page = document.load_pdf_page(0).unwrap();
    source_page
        .add_square_annotation(Rect::new(10.0, 10.0, 100.0, 100.0))
        .unwrap();
    drop(source_page);
    let mut annotated_pdf = Vec::new();
    document.write_to(&mut annotated_pdf).unwrap();

    let output = fit_pdf_to_a4(&annotated_pdf, 5.0).unwrap();
    let fitted = PdfDocument::from_bytes(&output).unwrap();
    let fitted_page = fitted.load_pdf_page(0).unwrap();

    assert_eq!(fitted_page.annotations().count(), 0);
}

#[test]
fn fitting_pdf_preserves_the_page_transparency_group() {
    let document = PdfDocument::from_bytes(NORMAL_PDF).unwrap();
    let page = document.load_pdf_page(0).unwrap();
    let mut group = document.new_dict().unwrap();
    group
        .dict_put("S", PdfObject::new_name("Transparency").unwrap())
        .unwrap();
    group
        .dict_put("CS", PdfObject::new_name("DeviceRGB").unwrap())
        .unwrap();
    page.object().dict_put("Group", group).unwrap();
    drop(page);
    let mut transparent_pdf = Vec::new();
    document.write_to(&mut transparent_pdf).unwrap();

    let output = fit_pdf_to_a4(&transparent_pdf, 5.0).unwrap();
    let fitted = PdfDocument::from_bytes(&output).unwrap();
    let fitted_page = fitted.load_pdf_page(0).unwrap();
    assert!(fitted_page.object().get_dict("Group").unwrap().is_none());
    let resources = fitted_page.object().get_dict("Resources").unwrap().unwrap();
    let xobjects = resources.get_dict("XObject").unwrap().unwrap();
    let form = xobjects.get_dict("FittedPage").unwrap().unwrap();
    let group = form.get_dict("Group").unwrap().unwrap();

    assert_eq!(
        group.get_dict("S").unwrap().unwrap().as_name().unwrap(),
        b"Transparency"
    );
}

#[test]
fn fitting_pdf_discards_source_print_boxes() {
    let document = PdfDocument::from_bytes(NORMAL_PDF).unwrap();
    let page = document.load_pdf_page(0).unwrap();
    let source_box = document.new_object_from_str("[50 50 500 700]").unwrap();
    let mut page_object = page.object();
    for name in ["BleedBox", "TrimBox", "ArtBox"] {
        page_object
            .dict_put(name, source_box.try_clone().unwrap())
            .unwrap();
    }
    drop(page);
    let mut boxed_pdf = Vec::new();
    document.write_to(&mut boxed_pdf).unwrap();

    let output = fit_pdf_to_a4(&boxed_pdf, 5.0).unwrap();
    let fitted = PdfDocument::from_bytes(&output).unwrap();
    let fitted_page = fitted.load_pdf_page(0).unwrap();
    let fitted_object = fitted_page.object();

    for name in ["BleedBox", "TrimBox", "ArtBox"] {
        assert!(fitted_object.get_dict(name).unwrap().is_none());
    }
}

#[test]
fn fitting_pdf_normalises_non_default_user_units() {
    let document = PdfDocument::from_bytes(NORMAL_PDF).unwrap();
    let page = document.load_pdf_page(0).unwrap();
    page.object()
        .dict_put("UserUnit", PdfObject::new_int(2).unwrap())
        .unwrap();
    drop(page);
    let mut large_units_pdf = Vec::new();
    document.write_to(&mut large_units_pdf).unwrap();

    let output = fit_pdf_to_a4(&large_units_pdf, 5.0).unwrap();
    let fitted_page = inspect_pdf(&output).unwrap().pages.remove(0);

    assert!(is_a4_size_mm(fitted_page.size.w_mm, fitted_page.size.h_mm));
}

#[test]
fn fitting_pdf_preserves_embedded_image_pixels() {
    let source = inspect_pdf(IMAGE_PDF).unwrap();
    let output = fit_pdf_to_a4(IMAGE_PDF, 5.0).unwrap();
    let fitted = inspect_pdf(&output).unwrap();
    let source_pixels: Vec<_> = source.pages[0]
        .images
        .iter()
        .map(|image| (image.pixel_width, image.pixel_height))
        .collect();
    let fitted_pixels: Vec<_> = fitted.pages[0]
        .images
        .iter()
        .map(|image| (image.pixel_width, image.pixel_height))
        .collect();

    assert_eq!(fitted_pixels, source_pixels);
}

#[test]
fn fitting_pdf_handles_mixed_portrait_and_landscape_pages() {
    let mut document = PdfDocument::from_bytes(NORMAL_PDF).unwrap();
    document.duplicate_page(0).unwrap();
    let mut landscape_page = document.load_pdf_page(1).unwrap();
    landscape_page.set_rotation(90).unwrap();
    drop(landscape_page);
    let mut mixed_pdf = Vec::new();
    document.write_to(&mut mixed_pdf).unwrap();

    let output = fit_pdf_to_a4(&mixed_pdf, 5.0).unwrap();
    let fitted = inspect_pdf(&output).unwrap();

    assert_eq!(fitted.pages.len(), 2);
    assert!(fitted.pages[0].size.h_mm > fitted.pages[0].size.w_mm);
    assert!(fitted.pages[1].size.w_mm > fitted.pages[1].size.h_mm);
    assert!(fitted
        .pages
        .iter()
        .all(|page| is_a4_size_mm(page.size.w_mm, page.size.h_mm)));
}

#[test]
fn fitting_landscape_pdf_preserves_landscape_orientation() {
    let document = PdfDocument::from_bytes(NORMAL_PDF).unwrap();
    let mut source_page = document.load_pdf_page(0).unwrap();
    source_page.set_rotation(90).unwrap();
    drop(source_page);
    let mut landscape_pdf = Vec::new();
    document.write_to(&mut landscape_pdf).unwrap();

    let output = fit_pdf_to_a4(&landscape_pdf, 5.0).unwrap();
    let page = &inspect_pdf(&output).unwrap().pages[0];

    assert!(page.size.w_mm > page.size.h_mm);
    assert!(is_a4_size_mm(page.size.w_mm, page.size.h_mm));
    assert!(!is_tight_to_edge(
        RectMm::new(0.0, 0.0, 297.0, 210.0),
        page.content_bbox.unwrap(),
        5.0,
    ));
}

# preflight-rs

Generic PDF preflight microservice. It accepts PDF uploads, runs structured
print-readiness checks, and returns JSON facts. It does not emit
business-specific or customer-facing text in API results.

## Licensing

This repository is licensed under AGPL-3.0-or-later.

The service links MuPDF through the `mupdf` crate and invokes Ghostscript. Both
are dual-licensed by Artifex under AGPL-3.0 or a paid commercial licence.
Because MuPDF is linked, this whole service is AGPL-licensed. Anyone running it
as a network service must offer the complete corresponding source code for this
service.

A separate application that calls this service over HTTP is not a derivative
work solely because of that arm's-length process boundary. The caller can have
its own licence; this repository remains AGPL.

Commercial licences for MuPDF and Ghostscript are available from Artifex.

## Requirements

- Rust toolchain
- C/C++ build tools for MuPDF
- Ghostscript available as `gs`

Debian/Ubuntu:

```bash
sudo apt-get update
sudo apt-get install -y build-essential clang cmake pkg-config ghostscript ca-certificates
```

macOS:

```bash
brew install rust ghostscript
```

## Configuration

Environment variables:

- `API_BEARER_TOKEN` required
- `BIND_ADDR` default `0.0.0.0:8080`
- `MAX_UPLOAD_BYTES` default `209715200`
- `MAX_PAGES` default `500`
- `MARGIN_MM` default `5`
- `MIN_DPI` default `150`
- `COLOUR_THRESHOLD` default `0.01`
- `COLOR_MODE` default `color`; use `mono` to convert uploads to grayscale before analysis
- `GS_CONCURRENCY` default logical CPU count
- `GS_BIN` default `gs`
- `GS_TIMEOUT_SECONDS` default `300`
- `CALLBACK_HOSTS` comma-separated allowlist required by `/pdf/process`
- `RUST_LOG` default from tracing subscriber

## Run

```bash
API_BEARER_TOKEN=secret cargo run
```

Docker:

```bash
API_BEARER_TOKEN=secret docker compose up --build
```

## Endpoints

All endpoints require `Authorization: Bearer <API_BEARER_TOKEN>`.

`GET /healthz`

Returns `ok`.

`GET /version`

Returns:

```json
{
  "name": "preflight-rs",
  "version": "0.1.0",
  "source_url": "https://github.com/cygnusdevs/preflight-rs",
  "license": "AGPL-3.0-or-later",
  "ghostscript_version": "10.07.1"
}
```

`POST /pdf/analyse`

Synchronous multipart upload. Requires `Authorization: Bearer <token>`.

```bash
curl -sS \
  -H "Authorization: Bearer secret" \
  -F "file=@document.pdf;type=application/pdf" \
  -F "max_pages=500" \
  -F "margin_mm=5" \
  -F "min_dpi=150" \
  -F "colour_threshold=0.01" \
  -F "color_mode=color" \
  http://localhost:8080/pdf/analyse
```

`color_mode=mono` converts the uploaded PDF to grayscale with Ghostscript before
inspection and ink coverage checks. The default `color` mode analyses the
uploaded PDF as-is.

`POST /pdf/process`

Asynchronous multipart upload. Requires `callback_url` and returns a job id.

```bash
curl -sS \
  -H "Authorization: Bearer secret" \
  -F "file=@document.pdf;type=application/pdf" \
  -F "callback_url=https://example.test/preflight-callback" \
  -F "callback_token=callback-secret" \
  http://localhost:8080/pdf/process
```

`POST /pdf/prepare`

Synchronous multipart upload. It returns a `multipart/mixed` response containing
the JSON preflight result and, when preflight succeeds, the analysed PDF.

```bash
curl -sS \
  -H "Authorization: Bearer secret" \
  -F "file=@document.pdf;type=application/pdf" \
  -F "max_pages=500" \
  -F "margin_mm=5" \
  -F "fit_to_page=true" \
  -F "color_mode=mono" \
  http://localhost:8080/pdf/prepare
```

When `fit_to_page=true`, every source page is scaled proportionally and centred
on an A4 page matching its portrait or landscape orientation inside the
requested `margin_mm`. Content is never cropped or stretched. The service uses
temporary files only while processing the request and does not persist uploads
or prepared PDFs.

## Result Schema

```json
{
  "schema_version": "3.0",
  "job_id": "uuid",
  "status": "completed",
  "source_file": {
    "bytes": 123,
    "sha256": "hex",
    "pdf_version": "1.7"
  },
  "analysed_file": {
    "bytes": 123,
    "sha256": "hex",
    "pdf_version": "1.7"
  },
  "analysis": {
    "color_mode": "color",
    "converted_to_grayscale": false,
    "fit_to_page": false,
    "fitted_to_page": false,
    "max_pages": 500,
    "margin_mm": 5.0,
    "min_dpi": 150.0,
    "colour_threshold": 0.01
  },
  "summary": {
    "pages": 1,
    "has_colour": false,
    "errors": 0,
    "warnings": 0
  },
  "pages": [
    {
      "page": 1,
      "size": {
        "w_mm": 210.0,
        "h_mm": 297.0,
        "is_a4": true
      },
      "margins": {
        "tight": false
      },
      "colour": {
        "has_colour": false,
        "coverage": {
          "c": 0.0,
          "m": 0.0,
          "y": 0.0,
          "k": 0.1
        }
      },
      "blank": false,
      "images": [
        {
          "pixel_width": 1200,
          "pixel_height": 1600,
          "placed": {
            "w_mm": 210.0,
            "h_mm": 297.0
          },
          "dpi": 145.14,
          "low_res": true
        }
      ]
    }
  ],
  "checks": [
    {
      "id": "pdf_valid",
      "severity": "error",
      "status": "pass",
      "data": {}
    }
  ]
}
```

Check ids are run in order: `pdf_valid`, `readable`, `encrypted`,
`page_count`, `page_dimensions`, `margins`, `colour`, `blank_pages`,
`image_resolution`.

## Callback Events

Step:

```json
{
  "job_id": "uuid",
  "event": "step",
  "step": "colour",
  "index": 7,
  "total": 9,
  "status": "pass",
  "data": {},
  "ts": "2026-06-29T12:00:00Z"
}
```

Completed:

```json
{
  "job_id": "uuid",
  "event": "completed",
  "result": {},
  "ts": "2026-06-29T12:00:00Z"
}
```

Failed:

```json
{
  "job_id": "uuid",
  "event": "failed",
  "result": {},
  "ts": "2026-06-29T12:00:00Z"
}
```

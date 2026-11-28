//! Streamed responses (NFR-7): NDJSON events and files.

use std::convert::Infallible;

use axum::body::Body;
use axum::http::{HeaderValue, header};
use axum::response::Response;
use bytes::Bytes;
use futures::stream;
use serde::Serialize;
use tokio::sync::mpsc;

use crate::service::table::ExportFormat;

/// One JSON object per line, as the events arrive.
pub fn ndjson<T: Serialize + Send + 'static>(rx: mpsc::Receiver<T>) -> Response {
    let body = stream::unfold(rx, |mut rx| async move {
        let event = rx.recv().await?;
        let mut line = serde_json::to_vec(&event).expect("events serialize");
        line.push(b'\n');
        Some((Ok::<_, Infallible>(Bytes::from(line)), rx))
    });
    let mut response = Response::new(Body::from_stream(body));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-ndjson"),
    );
    response
}

/// A file in `format`, downloaded as `<name>.<extension>`. An error after
/// the start aborts the transfer.
pub fn file(
    rx: mpsc::Receiver<Result<Bytes, std::io::Error>>,
    format: ExportFormat,
    name: &str,
) -> Response {
    let body = stream::unfold(rx, |mut rx| async move {
        let chunk = rx.recv().await?;
        Some((chunk, rx))
    });
    let mut response = Response::new(Body::from_stream(body));
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(format.content_type()),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{name}.{}\"",
            format.extension()
        ))
        .expect("ASCII header"),
    );
    response
}

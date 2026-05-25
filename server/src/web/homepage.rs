use axum::{
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};

const HOME_HTML: &str = include_str!("../../static/home/index.html");
const HOME_CSS: &str = include_str!("../../static/home/home.css");
const HOME_JS: &str = include_str!("../../static/home/home.js");

pub fn serve_home(path: &str) -> Option<Response> {
    let (content_type, body) = match path {
        "/" => ("text/html; charset=utf-8", HOME_HTML),
        "/__home/home.css" => ("text/css; charset=utf-8", HOME_CSS),
        "/__home/home.js" => ("application/javascript; charset=utf-8", HOME_JS),
        _ => return None,
    };

    let mut headers = HeaderMap::new();
    headers.insert("content-type", HeaderValue::from_static(content_type));

    Some((StatusCode::OK, headers, body).into_response())
}

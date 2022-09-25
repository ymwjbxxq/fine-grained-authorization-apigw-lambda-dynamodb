use lambda_http::{http::StatusCode, Response};
pub struct ApiHelper;

impl ApiHelper {
    pub fn response(
        status_code: StatusCode,
        body: String,
        content_type: String,
    ) -> Response<String> {
        Response::builder()
            .status(status_code)
            .header("Content-Type", content_type)
            .header("Access-Control-Allow-Origin", "*".to_string())
            .header("Access-Control-Allow-Headers", "Content-Type".to_string())
            .header(
                "Access-Control-Allow-Methods",
                "GET, POST, OPTIONS, PATCH, PUT, DELETE".to_string(),
            )
            .header("Access-Control-Allow-Credentials", "true".to_string())
            .body(body)
            .unwrap()
    }
}

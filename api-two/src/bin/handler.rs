use lambda_http::{self, http::StatusCode, service_fn, Error, IntoResponse, Request, RequestExt};
use serde_json::json;
use shared::utils::{api_helper::ApiHelper};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing_subscriber::filter::LevelFilter::INFO)
        .init();

    let _config = aws_config::load_from_env().await;
    // load clients like dynamodb or similar
    // set the DI to use the lambda context

    lambda_http::run(service_fn(|event: Request| execute(event))).await?;
    Ok(())
}

pub async fn execute(
    event: Request,
) -> Result<impl IntoResponse, Error> {
    println!("{:?}", event);

    // read the payload
    // let request = event.payload::<MyStruct>()?.unwrap();
    // do something with the payload

    Ok(ApiHelper::response(
        StatusCode::OK,
        json!({ "message": "authorized" }).to_string(),
        "application/json".to_string(),
    ))
}

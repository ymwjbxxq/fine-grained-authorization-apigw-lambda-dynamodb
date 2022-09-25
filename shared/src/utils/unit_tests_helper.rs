use aws_config;
use aws_sdk_dynamodb;
use aws_smithy_client::{erase::DynConnector, test_connection::TestConnection};
use aws_smithy_http::body::SdkBody;
use http::Request;
use lambda_http::Response;

pub struct UnitTestHelper {}

impl UnitTestHelper {
    pub fn buil_test_connnection(
        request: Option<Request<SdkBody>>,
        response: Option<Response<SdkBody>>,
    ) -> TestConnection<SdkBody> {
        let mut event = Self::default_event();
        if let (Some(request), Some(response)) = (request, response) {
            event.0 = request;
            event.1 = response;
        }
        TestConnection::new(vec![(event.0, event.1)])
    }

    fn default_event() -> (Request<SdkBody>, Response<SdkBody>) {
        let request = http::Request::new(SdkBody::from("request body"));
        let response = Response::builder()
            .status(200)
            .body(SdkBody::from(
                r#"{
                 "Item": {
                    "pk": {"S": "ciao"}
                    }
                  }"#,
            ))
            .unwrap();
        (request, response)
    }


    async fn dynamo_mock_config() -> aws_sdk_dynamodb::Config {
        let cfg = aws_config::from_env()
            .region(aws_sdk_dynamodb::Region::new("eu-central-1"))
            .credentials_provider(aws_sdk_dynamodb::Credentials::new(
                "accesskey",
                "privatekey",
                None,
                None,
                "dummy",
            ))
            .load()
            .await;

        aws_sdk_dynamodb::Config::new(&cfg)
    }

    pub fn dynamodb_request_builder() -> http::request::Builder {
        http::Request::builder()
            .header("content-type", "application/x-amz-json-1.0")
            .uri(http::uri::Uri::from_static(
                "https://dynamodb.eu-central-1.amazonaws.com/",
            ))
    }

    pub async fn dynamo_fake_client(conn: &TestConnection<SdkBody>) -> aws_sdk_dynamodb::Client {
        aws_sdk_dynamodb::Client::from_conf_conn(
            Self::dynamo_mock_config().await,
            DynConnector::new(conn.clone()),
        )
    }
}

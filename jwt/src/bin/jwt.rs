use aws_lambda_events::apigw::{
    ApiGatewayCustomAuthorizerRequestTypeRequest, ApiGatewayCustomAuthorizerResponse,
};
use lambda_request_authorizer::{
    queries::get_scope::GetScope,
    utils::{
        authorizer::Authorizer,
        injections::jwt_di::{JWTAppClient, JWTInitialisation},
    },
};
use lambda_runtime::{self, service_fn, Error, LambdaEvent};

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_max_level(tracing_subscriber::filter::LevelFilter::INFO)
        .init();

    let audience = std::env::var("AUDIENCE").expect("AUDIENCE must be set");
    let token_issuer = std::env::var("TOKEN_ISSUER").expect("TOKEN_ISSUER must be set");
    let json_key_set_url = std::env::var("JSKS_URI").expect("JSKS_URI must be set");

    let config = aws_config::load_from_env().await;
    let dynamodb_client = aws_sdk_dynamodb::Client::new(&config);
    let table_name = std::env::var("SCOPE_TABLE_NAME").expect("SCOPE_TABLE_NAME must be set");
    let query = GetScope::builder()
        .table_name(table_name)
        .dynamo_db_client(dynamodb_client.clone())
        .build();

    let app_client = JWTAppClient::builder()
        .authorizer(
            Authorizer::builder()
                .json_key_set_url(json_key_set_url)
                .audience(audience)
                .issuer(token_issuer)
                .reqwest_client(reqwest::Client::new())
                .build(),
        )
        .get_scope_query(query)
        .build();

    lambda_runtime::run(service_fn(
        |event: LambdaEvent<ApiGatewayCustomAuthorizerRequestTypeRequest>| {
            execute(&app_client, event)
        },
    ))
    .await?;
    Ok(())
}

pub async fn execute(
    app_client: &dyn JWTInitialisation,
    event: LambdaEvent<ApiGatewayCustomAuthorizerRequestTypeRequest>,
) -> Result<ApiGatewayCustomAuthorizerResponse, Error> {
    println!("event {:?}", event);
    let method_arn = event.payload.method_arn.unwrap();
    let path = event.payload.path.unwrap();
    let method = event.payload.http_method.unwrap_or_default().to_string();
    if let Some(token) = event.payload.headers.get("authorization") {
        let claims = app_client
            .validate_token(token.to_str().unwrap().to_string())
            .await?;
        if let Some(claims) = claims {
            if let Some(token_scope) = claims.scope {
                let api_scopes = app_client.get_scope_query(&method, &path).await?;
                if let Some(api_scopes) = api_scopes {
                    for api_scope in api_scopes {
                        if token_scope.split(' ').any(|x| x == api_scope.as_str()) {
                            return Ok(app_client.to_response(
                                "ALLOW".to_string(),
                                Some(claims.email),
                                method_arn,
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(app_client.to_response("DENY".to_string(), None, method_arn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use aws_lambda_events::apigw::{
        ApiGatewayCustomAuthorizerPolicy, ApiGatewayCustomAuthorizerRequestTypeRequest,
        ApiGatewayCustomAuthorizerResponse, IamPolicyStatement,
    };
    use lambda_http::Context;
    use lambda_request_authorizer::utils::authorizer::Claims;
    use mockall::mock;
    use serde_json::{self, Value};
    use shared::error::ApplicationError;

    fn get_lambda_request() -> LambdaEvent<ApiGatewayCustomAuthorizerRequestTypeRequest> {
        let json = r#"{
  "type": "REQUEST",
  "methodArn": "arn:aws:execute-api:us-east-1:123456789012:abcdef123/one/GET",
  "resource": "/request",
  "path": "/one/",
  "httpMethod": "GET",
  "headers": {
    "authorization": "Bearer token",
    "X-AMZ-Date": "20170718T062915Z",
    "Accept": "*/*",
    "HeaderAuth1": "headerValue1",
    "CloudFront-Viewer-Country": "US",
    "CloudFront-Forwarded-Proto": "https",
    "CloudFront-Is-Tablet-Viewer": "false",
    "CloudFront-Is-Mobile-Viewer": "false",
    "User-Agent": "..."
  },
  "queryStringParameters": {
    "QueryString1": "queryValue1"
  },
  "pathParameters": {},
  "stageVariables": {
    "StageVar1": "stageValue1"
  },
  "requestContext": {
    "path": "/request",
    "accountId": "123456789012",
    "resourceId": "05c7jb",
    "stage": "test",
    "requestId": "...",
    "identity": {
      "apiKey": "...",
      "sourceIp": "...",
      "clientCert": {
        "clientCertPem": "CERT_CONTENT",
        "subjectDN": "www.example.com",
        "issuerDN": "Example issuer",
        "serialNumber": "a1:a1:a1:a1:a1:a1:a1:a1:a1:a1:a1:a1:a1:a1:a1:a1",
        "validity": {
          "notBefore": "May 28 12:30:02 2019 GMT",
          "notAfter": "Aug  5 09:36:04 2021 GMT"
        }
      }
    },
    "resourcePath": "/request",
    "httpMethod": "GET",
    "apiId": "abcdef123"
  }
}"#;

        let request: ApiGatewayCustomAuthorizerRequestTypeRequest =
            serde_json::from_str(json).unwrap();

        let context = Context::default();
        let event = LambdaEvent::new(request, context);

        event
    }

    #[tokio::test]
    async fn will_allow() -> Result<(), ApplicationError> {
        // ARRANGE
        mock! {
            pub JWTAppClient {}
            #[async_trait]
            impl JWTInitialisation for JWTAppClient {
                async fn validate_token(&self, raw_token: String) -> Result<Option<Claims>, ApplicationError>;
                fn to_response(&self, effect: String, principal: Option<String>, method_arn: String) -> ApiGatewayCustomAuthorizerResponse;
                async fn get_scope_query(&self, method: &str, path: &str) -> Result<Option<Vec<String>>, ApplicationError>;
            }
        }

        let mut mock = MockJWTAppClient::default();
        mock.expect_validate_token().times(1).returning(|_| {
            let data = r#"
                    {
              "exp": 1654242297,
              "iss": "https://somedomain.com",
              "aud": "my-audience",
              "sub": "12408bde-207d-45a5-a143-6aa02f049df7",
              "scope": "my-audience.my-custom-scope",
              "email": "a@a.com"
            }"#;
            let v: Value = serde_json::from_str(data)?;
            let response: Claims = serde_json::from_value(v)?;
            Ok(Some(response))
        });
        mock.expect_get_scope_query().times(1).returning(|_, _| {
            Ok(Some(vec![
                "my-audience.my-custom-scope".to_string(),
                "something".to_string(),
            ]))
        });
        mock.expect_to_response().times(1).returning(|_, _, _| {
            let stmt = IamPolicyStatement {
                action: vec!["execute-api:Invoke".to_string()],
                resource: vec!["something".to_string()],
                effect: Some("ALLOW".to_string()),
            };
            let policy = ApiGatewayCustomAuthorizerPolicy {
                version: Some("2012-10-17".to_string()),
                statement: vec![stmt],
            };

            let response = ApiGatewayCustomAuthorizerResponse {
                principal_id: Some("something".to_string()),
                policy_document: policy,
                context: Value::Null,
                usage_identifier_key: None,
            };

            response
        });

        // ACT
        let result = execute(&mock, get_lambda_request()).await?;

        // // ASSERT
        let json = serde_json::to_string(&result).expect("failed to serialize to json");

        assert_eq!(
            "{\"principalId\":\"something\",\"policyDocument\":{\"Version\":\"2012-10-17\",\"Statement\":[{\"Action\":[\"execute-api:Invoke\"],\"Effect\":\"ALLOW\",\"Resource\":[\"something\"]}]},\"context\":null,\"usageIdentifierKey\":null}",
            json
        );

        Ok(())
    }

    #[tokio::test]
    async fn will_deny_when_authorization_token_is_not_passed() -> Result<(), ApplicationError> {
        // ARRANGE
        mock! {
            pub JWTAppClient {}
            #[async_trait]
            impl JWTInitialisation for JWTAppClient {
                async fn validate_token(&self, raw_token: String) -> Result<Option<Claims>, ApplicationError>;
                fn to_response(&self, effect: String, principal: Option<String>, method_arn: String) -> ApiGatewayCustomAuthorizerResponse;
                async fn get_scope_query(&self, method: &str, path: &str) -> Result<Option<Vec<String>>, ApplicationError>;
            }
        }

        let mut mock = MockJWTAppClient::default();
        mock.expect_validate_token().times(0);
        mock.expect_to_response().times(1).returning(|_, _, _| {
            let stmt = IamPolicyStatement {
                action: vec!["execute-api:Invoke".to_string()],
                resource: vec!["something".to_string()],
                effect: Some("DENY".to_string()),
            };
            let policy = ApiGatewayCustomAuthorizerPolicy {
                version: Some("2012-10-17".to_string()),
                statement: vec![stmt],
            };

            let response = ApiGatewayCustomAuthorizerResponse {
                principal_id: None,
                policy_document: policy,
                context: Value::Null,
                usage_identifier_key: None,
            };

            response
        });

        let mut request = get_lambda_request();
        request.payload.headers.clear();

        // ACT
        let result = execute(&mock, request).await?;

        // // ASSERT
        let json = serde_json::to_string(&result).expect("failed to serialize to json");

        assert_eq!(
            "{\"principalId\":null,\"policyDocument\":{\"Version\":\"2012-10-17\",\"Statement\":[{\"Action\":[\"execute-api:Invoke\"],\"Effect\":\"DENY\",\"Resource\":[\"something\"]}]},\"context\":null,\"usageIdentifierKey\":null}",
            json
        );

        Ok(())
    }

    #[tokio::test]
    async fn will_deny_when_validate_token_does_not_return_claims() -> Result<(), ApplicationError>
    {
        // ARRANGE
        mock! {
            pub JWTAppClient {}
            #[async_trait]
            impl JWTInitialisation for JWTAppClient {
                async fn validate_token(&self, raw_token: String) -> Result<Option<Claims>, ApplicationError>;
                fn to_response(&self, effect: String, principal: Option<String>, method_arn: String) -> ApiGatewayCustomAuthorizerResponse;
                async fn get_scope_query(&self, method: &str, path: &str) -> Result<Option<Vec<String>>, ApplicationError>;
            }
        }

        let mut mock = MockJWTAppClient::default();
        mock.expect_validate_token()
            .times(1)
            .returning(|_| Ok(None));
        mock.expect_to_response().times(1).returning(|_, _, _| {
            let stmt = IamPolicyStatement {
                action: vec!["execute-api:Invoke".to_string()],
                resource: vec!["something".to_string()],
                effect: Some("DENY".to_string()),
            };
            let policy = ApiGatewayCustomAuthorizerPolicy {
                version: Some("2012-10-17".to_string()),
                statement: vec![stmt],
            };

            let response = ApiGatewayCustomAuthorizerResponse {
                principal_id: None,
                policy_document: policy,
                context: Value::Null,
                usage_identifier_key: None,
            };

            response
        });

        // ACT
        let result = execute(&mock, get_lambda_request()).await?;

        // // ASSERT
        let json = serde_json::to_string(&result).expect("failed to serialize to json");

        assert_eq!(
            "{\"principalId\":null,\"policyDocument\":{\"Version\":\"2012-10-17\",\"Statement\":[{\"Action\":[\"execute-api:Invoke\"],\"Effect\":\"DENY\",\"Resource\":[\"something\"]}]},\"context\":null,\"usageIdentifierKey\":null}",
            json
        );

        Ok(())
    }
}

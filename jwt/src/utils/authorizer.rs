use async_trait::async_trait;
use aws_lambda_events::apigw::{
    ApiGatewayCustomAuthorizerPolicy, ApiGatewayCustomAuthorizerResponse, IamPolicyStatement,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{self, Value};
use shared::error::ApplicationError;
use typed_builder::TypedBuilder as Builder;

#[async_trait]
pub trait JWTAuthorizer {
    async fn validate_token(&self, raw_token: String) -> Result<Option<Claims>, ApplicationError>;
    async fn get_jwks_key(&self, kid: &str) -> Result<Option<JwtKey>, ApplicationError>;
}

#[derive(Deserialize)]
pub struct JwtKeys {
    pub keys: Vec<JwtKey>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JwtKey {
    pub e: String,
    pub kty: String,
    pub alg: Option<String>,
    pub n: String,
    pub kid: String,
}

impl Clone for JwtKey {
    fn clone(&self) -> Self {
        JwtKey {
            e: self.e.clone(),
            kty: self.kty.clone(),
            alg: self.alg.clone(),
            n: self.n.clone(),
            kid: self.kid.clone(),
        }
    }
}

#[derive(Debug, Clone, Builder, Default)]
pub struct Authorizer {
    #[builder(setter(into))]
    pub json_key_set_url: String,

    #[builder(setter(into))]
    pub audience: String,

    #[builder(setter(into))]
    pub issuer: String,

    pub reqwest_client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub aud: String,
    pub sub: String,
    pub email: String,
    pub exp: usize,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_access: Option<ResourceAccess>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceAccess {
    #[serde(rename = "my-audience")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<App>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct App {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "role")]
    pub roles: Option<Vec<String>>,
}

impl Authorizer {
    fn get_token(&self, raw_token: String) -> Option<String> {
        let token = raw_token.strip_prefix("Bearer ");

        token.map(str::to_string)
    }

    pub fn to_response(
        &self,
        effect: String,
        principal: Option<String>,
        method_arn: String,
    ) -> ApiGatewayCustomAuthorizerResponse {
        let stmt = IamPolicyStatement {
            action: vec!["execute-api:Invoke".to_string()],
            resource: vec![method_arn],
            effect: Some(effect),
        };
        let policy = ApiGatewayCustomAuthorizerPolicy {
            version: Some("2012-10-17".to_string()),
            statement: vec![stmt],
        };

        ApiGatewayCustomAuthorizerResponse {
            principal_id: principal,
            policy_document: policy,
            context: Value::Null,
            usage_identifier_key: None,
        }
    }
}

#[async_trait]
impl JWTAuthorizer for Authorizer {
    async fn get_jwks_key(&self, kid: &str) -> Result<Option<JwtKey>, ApplicationError> {
        let res = self
            .reqwest_client
            .get(&self.json_key_set_url)
            .send()
            .await?;
        let jwks = res.json::<JwtKeys>().await?;

        let jwt_key = jwks.keys.into_iter().find(|x| x.kid == kid);

        Ok(jwt_key)
    }

    async fn validate_token(&self, raw_token: String) -> Result<Option<Claims>, ApplicationError> {
        if let Some(token) = self.get_token(raw_token) {
            if let Ok(header) = decode_header(&token) {
                if let Some(kid) = header.kid {
                    let jwt_key = self.get_jwks_key(&kid).await?;
                    if let Some(jwk) = jwt_key {
                        let mut validation = Validation::new(Algorithm::RS256);
                        validation.set_audience(&[&self.audience]);
                        validation.set_issuer(&[&self.issuer]);

                        let result = decode::<Value>(
                            &token,
                            &DecodingKey::from_rsa_components(&jwk.n, &jwk.e)?,
                            &validation,
                        );

                        if let Ok(token_data) = result {
                            let claims: Claims = serde_json::from_value(token_data.claims)?;
                            return Ok(Some(claims));
                        }
                    }
                }
            }
        }
        //Invalid Authorization token - ${jwtTokenStr} does not match "Bearer .*

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;
    use serde_json::Value;

    #[test]
    fn strip_prefix_token() -> Result<(), ApplicationError> {
        // ARRANGE
        let raw_token = r#"Bearer token"#;
        let authorizer = Authorizer::default();

        // ACT
        let token = authorizer.get_token(raw_token.to_string());

        // ASSERT
        assert_eq!(token, Some("token".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn get_the_correct_jwks_kid() -> Result<(), ApplicationError> {
        // ARRANGE
        let _m = mock("GET", "/endpoint")
            // .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{\"keys\":[{\"kid\":\"first\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"token\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"9kbxqJiC1PUWEutioScurzlsjWY\",\"x5t#S256\":\"uVcZew1d60ora1g_3HHb10I5wIMkFMA_XrTdF0SlGrc\"},{\"kid\":\"second\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"token\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"SpIpmWVAa7O7899MMvdOFIOIW2c\",\"x5t#S256\":\"NIAW_NdkPrTEjm1v9Ee0lrc12EDx7E-0MbzKZrrGcrI\"}]}")
            .create();

        let mut authorizer = Authorizer::default();
        authorizer.json_key_set_url = format!("{}/endpoint", mockito::server_url());

        // ACT
        let key = authorizer.get_jwks_key("second").await?;

        // ASSERT
        assert!(key.is_some());
        assert!(key.unwrap().kid == "second");

        Ok(())
    }

    #[tokio::test]
    async fn get_none_when_jwks_kid_not_found() -> Result<(), ApplicationError> {
        // ARRANGE
        let _m = mock("GET", "/endpoint")
            // .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{\"keys\":[{\"kid\":\"first\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"token\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"9kbxqJiC1PUWEutioScurzlsjWY\",\"x5t#S256\":\"uVcZew1d60ora1g_3HHb10I5wIMkFMA_XrTdF0SlGrc\"},{\"kid\":\"second\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"token\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"SpIpmWVAa7O7899MMvdOFIOIW2c\",\"x5t#S256\":\"NIAW_NdkPrTEjm1v9Ee0lrc12EDx7E-0MbzKZrrGcrI\"}]}")
            .create();

        let mut authorizer = Authorizer::default();
        authorizer.json_key_set_url = format!("{}/endpoint", mockito::server_url());

        // ACT
        let key = authorizer.get_jwks_key("third").await?;

        // ASSERT
        assert!(key.is_none());

        Ok(())
    }

    #[test]
    fn serde_claim() -> Result<(), ApplicationError> {
        // ARRANGE
        let data = r#"
        {
  "exp": 1654242297,
  "iss": "https://domain.com/issuer",
  "aud": "my-audience",
  "sub": "12408bde-207d-45a5-a143-6aa02f049df7",
  "resource_access": {
    "my-audience": {
      "role": [
        "some-role"
      ]
    }
  },
  "email": "a@a.com"
}"#;

        // ACT
        let v: Value = serde_json::from_str(data)?;
        let result: Claims = serde_json::from_value(v)?;

        // ASSERT
        assert!(result.resource_access.unwrap().app.unwrap().roles.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn return_none_when_token_is_not_valid() -> Result<(), ApplicationError> {
        // ARRANGE
        let token = "Bearer iLCJhY3IiOiIwIiwicmVzb3VyY2VfYWNjZXNzIjp7InJlZGlyZWN0LXVpIjp7InJvbGVzIjpbInJlZGlyZWN0LXVpLXVzZXIiXX19LCJzY29wZSI6InJlZGlyZWN0LXVpLnJlYWQgcHJvZmlsZSByZWRpcmVjdC11aS53cml0ZSBlbWFpbCIsInNpZCI6IjhkZDI2ZjIxLWY3MTAtNGNkYy05YTUzLTg4Yzg4YzNjOGViNCIsInJlc291cmNlX2FjY2VzcyI6eyJyZWRpcmVjdC11aSI6eyJyb2xlIjpbInJlZGlyZWN0LXVpLXVzZXIiXX19LCJlbWFpbF92ZXJpZmllZCI6ZmFsc2UsIm5hbWUiOiJEYW5pZWxlIEZyYXNjYSIsInByZWZlcnJlZF91c2VybmFtZSI6ImRmcmFzY2EiLCJnaXZlbl9uYW1lIjoiRGFuaWVsZSIsImZhbWlseV9uYW1lIjoiRnJhc2NhIiwiZW1haWwiOiJkYW5pZWxlLmZyYXNjYUBwcm9zaWViZW5zYXQxZGlnaXRhbC5kZSJ9.HNRLgnTOCiSdV70vEoKlaTn14-Nl7eFwuiFHEC_8LvV2bOiOSHS8LWgsdqL1KDnZTWmdxoNKfr-21xAwfyFFKBoNmHbSq_wLtElwa760pJpFLv6LfazOk4nCw9qlWuReqII4euevsGC1RWV5bn9oluiHB_JrrpoJqpR6S60p4vgYZneW0i1FPyZ5SHt72yVa1Uza0chM0JQlaqINBMJXQPgACBQHLsEnPI-qqo1j0I4cc3_qkUaizXXuVtnmSasPu05AwB_JGaGMNZzko_9cUBxmmGBtWLFrw3XjoCGUJa61AzGSX5d6rJu5x_6xIaPVzDjG6w1dm7eBE8fzLdVuzcYGmTmQlM9hS8j2s0sYmyUCBl2FkP8QJ3t41VrWuTZXhvclyQM0p-UkxQZ9fV99IJSXMhJZPTDYK9PYeAEfROSl_8Mt26aFkRPF71CDLA1d5P55k-EcR5NdqcsQ8DLeXYTSA6uRsRJgIhv1Np3EpMf5Z4gnMfOqXt4yZzMvKhMhblgrBODdeGZ_zu38PcnioI04vEdZMIDpLhbRW5_8jmLrhrg57kslkRXExCDfzmjaeDkuIu6B_sgNna74LM76lVYSOTPruah9jy4U33donsxHYvWefFhVJhxyIC7IheIKdSVG_VI-DwFHhyv6S4LUUJL4LfUZWlGAEeLvfeG31UM";
        let _m = mock("GET", "/endpoint")
            // .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{\"keys\":[{\"kid\":\"L1xwp8ksGmDmLViFLlKpuxYkD_6sAWnhi6Wb1YPWu3g\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"vx__0B89Tvtv6J1QZywUyBjFAfu1PBE3RFYtn6f3kvpkNSf-DAIy2FUrrI2Dm2HaYob53yOoMwJfHD-WuiFYaNPY7EhdTE-r-t2zfjEsw2eoUeHha32L9Mhn-yT5dkMW952YHy5XI6RyHbP4AoxGjmtduCq2zz3skhE99rFr6Az-DyETGrvIjoi5DLrDXaE71uKxp1To69BQpphpLFS90sszJ8QJXULr8URLWqMmt8RhLsalFOk7apGCB94wtT8M3IjESkGFZ449LbOwY2wa8ZjBFgAQY_iQNUGawAxPjqoO5uasD-YiUnsSdW7QbKxV3ClrmbYx4sc3UCfGyC76kFVue7-OiJDy2oIWQuZGQN3MwMNqjmHqy-qsmlCEeF1LCzb9gb0JsUUngdsET9LDUaX2-_i4l6ezfBEKf3KPrtVfXVmqpcUvS4EaL-08j9wvKwIoVr3nHLtrs-YUuzSo3IE5aZQCjippKZ3MG5IfJhIPy68b8wSK5WAwR0ixC2jh4UrveSqrxcL2YXGVq6SmspNmSVTqd7M_s__01px7dukVEB7gRHVb_TGOJ39XtY2W5VJd0y2lhGWHxgvBQzLvtGJuedoFy_Rgt_4W_ldaIBUBsJ2E8DEa11hM4562bSTWhRReqDwJIwcY12hlwYZNvnLlQp1L5gh5LXYTXTJVw3s\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"9kbxqJiC1PUWEutioScurzlsjWY\",\"x5t#S256\":\"uVcZew1d60ora1g_3HHb10I5wIMkFMA_XrTdF0SlGrc\"},{\"kid\":\"second\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"token\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"SpIpmWVAa7O7899MMvdOFIOIW2c\",\"x5t#S256\":\"NIAW_NdkPrTEjm1v9Ee0lrc12EDx7E-0MbzKZrrGcrI\"}]}")
            .create();

        let mut authorizer = Authorizer::default();
        authorizer.json_key_set_url = format!("{}/endpoint", mockito::server_url());

        // ACT
        let result = authorizer.validate_token(token.to_string()).await?;

        // ASSERT
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn return_none_when_token_is_not_decoded() -> Result<(), ApplicationError> {
        // ARRANGE
        let token = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c";
        let _m = mock("GET", "/endpoint")
            // .with_status(200)
            .with_header("content-type", "application/json")
            .with_body("{\"keys\":[{\"kid\":\"L1xwp8ksGmDmLViFLlKpuxYkD_6sAWnhi6Wb1YPWu3g\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"vx__0B89Tvtv6J1QZywUyBjFAfu1PBE3RFYtn6f3kvpkNSf-DAIy2FUrrI2Dm2HaYob53yOoMwJfHD-WuiFYaNPY7EhdTE-\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"9kbxqJiC1PUWEutioScurzlsjWY\",\"x5t#S256\":\"uVcZew1d60ora1g_3HHb10I5wIMkFMA_XrTdF0SlGrc\"},{\"kid\":\"second\",\"kty\":\"RSA\",\"alg\":\"RS256\",\"use\":\"sig\",\"n\":\"token\",\"e\":\"AQAB\",\"x5c\":[\"token\"],\"x5t\":\"SpIpmWVAagfsg45MMvdOFIOIW2c\",\"x5t#S256\":\"NIAW_dgfdgfdgdv9Efghgfe0lrc12EDx7E-0MbzKZrrGcrI\"}]}")
            .create();

        let mut authorizer = Authorizer::default();
        authorizer.json_key_set_url = format!("{}/endpoint", mockito::server_url());

        // ACT
        let result = authorizer.validate_token(token.to_string()).await?;

        // ASSERT
        assert!(result.is_none());

        Ok(())
    }
}

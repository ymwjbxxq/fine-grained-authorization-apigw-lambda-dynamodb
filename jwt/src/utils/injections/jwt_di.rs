use crate::{
    queries::get_scope::{GetScope, GetScopeQuery},
    utils::authorizer::{Authorizer, Claims, JWTAuthorizer},
};
use async_trait::async_trait;
use aws_lambda_events::apigw::ApiGatewayCustomAuthorizerResponse;
#[cfg(test)]
use mockall::{automock, predicate::*};
use shared::error::ApplicationError;
use typed_builder::TypedBuilder as Builder;

#[cfg_attr(test, automock)]
#[async_trait]
pub trait JWTInitialisation: Send + Sync {
    async fn validate_token(&self, raw_token: String) -> Result<Option<Claims>, ApplicationError>;
    fn to_response(
        &self,
        effect: String,
        principal: Option<String>,
        method_arn: String,
    ) -> ApiGatewayCustomAuthorizerResponse;
    async fn get_scope_query(
        &self,
        method: &str,
        path: &str,
    ) -> Result<Option<Vec<String>>, ApplicationError>;
}

#[derive(Debug, Clone, Builder)]
pub struct JWTAppClient {
    #[builder(setter(into))]
    pub authorizer: Authorizer,

    #[builder(setter(into))]
    pub get_scope_query: GetScope,
}

#[async_trait]
impl JWTInitialisation for JWTAppClient {
    fn to_response(
        &self,
        effect: String,
        principal: Option<String>,
        method_arn: String,
    ) -> ApiGatewayCustomAuthorizerResponse {
        self.authorizer.to_response(effect, principal, method_arn)
    }

    async fn validate_token(&self, raw_token: String) -> Result<Option<Claims>, ApplicationError> {
        self.authorizer.validate_token(raw_token).await
    }

    async fn get_scope_query(
        &self,
        method: &str,
        path: &str,
    ) -> Result<Option<Vec<String>>, ApplicationError> {
        let mut application_identity = path.to_string();
        if !path.ends_with('/') {
            application_identity = format!("{}/", &path);
        }
        let api = format!("{}{}", method, application_identity);
        self.get_scope_query.execute(&api).await
    }
}

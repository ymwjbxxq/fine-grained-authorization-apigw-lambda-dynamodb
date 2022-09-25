use async_trait::async_trait;
use aws_sdk_dynamodb::{self, model::AttributeValue};
use shared::{error::ApplicationError, utils::dynamodb::AttributeValuesExt};
use typed_builder::TypedBuilder as Builder;

#[async_trait]
pub trait GetScopeQuery {
    async fn execute(&self, api: &str) -> Result<Option<Vec<String>>, ApplicationError>;
}

#[derive(Debug, Clone, Builder)]
pub struct GetScope {
    #[builder(setter(into))]
    table_name: String,

    #[builder(default, setter(strip_option))]
    pub dynamo_db_client: Option<aws_sdk_dynamodb::Client>,
}

#[async_trait]
impl GetScopeQuery for GetScope {
    async fn execute(&self, api: &str) -> Result<Option<Vec<String>>, ApplicationError> {
        let result = self
            .dynamo_db_client
            .as_ref()
            .unwrap()
            .get_item()
            .table_name(&self.table_name)
            .key("pk", AttributeValue::S(api.to_owned()))
            .send()
            .await?;

        Ok(result
            .item
            .map(|item| item.get_array_string("scopes").unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_smithy_http::body::SdkBody;
    use lambda_http::Response;
    use shared::utils::unit_tests_helper::UnitTestHelper;
    use tokio;

    #[tokio::test]
    async fn return_some_if_scope_found() -> Result<(), ApplicationError> {
        // ARRANGE
        let request = UnitTestHelper::dynamodb_request_builder();
        let request = request
            .header("x-amz-target", "DynamoDB_20120810.GetItem")
            .body(SdkBody::from(
                r#"{
                      "TableName":"some-table",
                      "Key":{
                        "pk":{"S":"pk_value"},
                        "scopes": {
                          "L": [
                            {
                              "S": "scope1"
                            },
                            {
                              "S": "scope2"
                            }
                          ]
                        }
                      }
                    }"#,
            ))
            .unwrap();
        let response = Response::builder()
            .status(200)
            .body(SdkBody::from(
                r#"{
                  "Item": {
                    "pk": {"S": "pk_value"}, 
                    "scopes": {
                          "L": [
                            {
                              "S": "scope1"
                            },
                            {
                              "S": "scope2"
                            }
                          ]
                        }
                    }
                  }"#,
            ))
            .unwrap();
        let conn = UnitTestHelper::buil_test_connnection(Some(request), Some(response));
        let dynamo_db_client = UnitTestHelper::dynamo_fake_client(&conn).await;

        let query = GetScope::builder()
            .table_name("some-table".to_owned())
            .dynamo_db_client(dynamo_db_client)
            .build();

        // ACT
        let result = query.execute("pk_value").await?;

        // ASSERT
        assert_eq!(result.unwrap().len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn return_none_if_scope_not_found() -> Result<(), ApplicationError> {
        // ARRANGE
        let request = UnitTestHelper::dynamodb_request_builder();
        let request = request
            .header("x-amz-target", "DynamoDB_20120810.GetItem")
            .body(SdkBody::from(
                r#"{
                      "TableName":"some-table",
                      "Key":{
                        "pk":{"S":"pk_value"},
                        "scopes": {
                          "L": [
                            {
                              "S": "scope1"
                            },
                            {
                              "S": "scope2"
                            }
                          ]
                        }
                      }
                    }"#,
            ))
            .unwrap();
        let response = Response::builder()
            .status(200)
            .body(SdkBody::from(
                r#"{
                  "Item": null
                  }"#,
            ))
            .unwrap();
        let conn = UnitTestHelper::buil_test_connnection(Some(request), Some(response));
        let dynamo_db_client = UnitTestHelper::dynamo_fake_client(&conn).await;

        let query = GetScope::builder()
            .table_name("some-table".to_owned())
            .dynamo_db_client(dynamo_db_client)
            .build();

        // ACT
        let result = query.execute("pk_value").await?;

        // ASSERT
        assert!(result.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn given_a_dynamodb_error_return_error() -> Result<(), ApplicationError> {
        // ARRANGE
        let request = UnitTestHelper::dynamodb_request_builder();
        let request = request
            .header("x-amz-target", "DynamoDB_20120810.GetItem")
            .body(SdkBody::from(
                r#"{
                      "TableName":"some-table",
                      "Key":{
                        "pk":{"S":"pk_value"},
                        "scopes": {
                          "L": [
                            {
                              "S": "scope1"
                            },
                            {
                              "S": "scope2"
                            }
                          ]
                        }
                      }
                    }"#,
            ))
            .unwrap();
        let response = Response::builder()
            .status(400)
            .body(SdkBody::from("{}"))
            .unwrap();
        let conn = UnitTestHelper::buil_test_connnection(Some(request), Some(response));
        let dynamo_db_client = UnitTestHelper::dynamo_fake_client(&conn).await;

        let query = GetScope::builder()
            .table_name("some-table".to_owned())
            .dynamo_db_client(dynamo_db_client)
            .build();

        // ACT
        let result = query.execute("pk_value").await;

        // ASSERT
        assert!(result.is_err());

        Ok(())
    }
}

# Building fine-grained authorization using any Identity and access management with API Gateway

Imagine a customer-facing application where users will log into your web or mobile application. As such, you will expose your APIs through API Gateway with upstream services. For instance, the user should be allowed to make a GET request to an endpoint but should not be allowed to make a POST request to the same endpoint. As a best practice, you should assign users fine scopes to allow or deny access to your API services.

Let's go through the request flow to understand what happens at each step, as shown in the figure (TODO):

1. A user logs into the  Identity and access management and acquires a JWT ID token, access token etc.
2. A RestAPI request is made, and a bearer token—in this solution, an access token—is passed in the headers.
3. API Gateway forwards the request to the LambdaRequestAuthorizer.
4. LambdaRequestAuthorizer verifies JWT using the Identity and access management provider. 
5. LambdaRequestAuthorizer looks up into Amazon DynamoDB the scope based on the custom domain path and method /one/get/ or /one/post
6. LambdaRequestAuthorizer return ALLOW or DENY.
7. The API Gateway policy engine evaluates the policy
8. The request is forwarded to the service.

## DynamoDB Scope table

We should have a DynamoDB table made of scopes. For example: 

 ``` 
{
 "pk": "GET/one",
 "scopes": [
  "my-audience.read"
 ]
}

{
 "pk": "POST/two",
 "scopes": [
  "my-audience.write"
 ]
}
 ```
## Project structure

 ``` 
.
├── API 1 (api-one)/
├── API 2 (api-two)/
├── Shared code (shared)/
├── LambdaRequestAuthorizer (jwt)/
└── cargo.toml

 ``` 

## Unit-tests

From the root, you can run either:
    ```
    cargo test
    ```
or
    ```
    make unit-tests
    ```

## Requirements
* [Create an AWS account](https://portal.aws.amazon.com/gp/aws/developer/registration/index.html) if you do not already have one and log in. The IAM user you use must have sufficient permissions to make AWS service calls and manage AWS resources.
* [AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/install-cliv2.html) installed and configured
* [Git Installed](https://git-scm.com/book/en/v2/Getting-Started-Installing-Git)
* [AWS Serverless Application Model](https://docs.aws.amazon.com/serverless-application-model/latest/developerguide/serverless-sam-cli-install.html) (AWS SAM) installed
* [Rust](https://www.rust-lang.org/) 1.64.0 or higher
* [cargo-zigbuild](https://github.com/messense/cargo-zigbuild) and [Zig](https://ziglang.org/) for cross-compilation
* [nextest](https://github.com/nextest-rs/nextest) Nextest is a next-generation test runner for Rust.

## Deployment Instructions

**ASSUMPTION:**
1. DynamoDB Scope table is present
2. Custom domain certificate is present
3. Create a Route 53 alias record that routes traffic to the custom domain

1. Create a new directory, navigate to that directory in a terminal and clone the GitHub repository:
    ``` 
    git clone https://github.com/ymwjbxxq/fine-grained-authorization-apigw-lambda-dynamodb.git
    ```
2. Change the directory to the pattern directory:
    ```
    cd fine-grained-authorization-apigw-lambda-dynamodb.git
    ```
3. Deploy the LambdaRequestAuthorizer:
    ```
    cd jwt
    make build
    make deploy
    ```
4. Deploy the api-one:
    ```
    cd api-one
    make build
    make deploy
    ```
5. Deploy the api-two:
    ```
    cd api-two
    make build
    make deploy
    ```
6. Deploy the custom domain:
    ```
    sam deploy --guided --no-fail-on-empty-changeset --no-confirm-changeset --stack-name myproject-customdomain --template-file ./custom-domain.yml
    ```
5. During the prompts:
    * Enter a stack name
    * Enter the desired AWS Region
    * Allow SAM CLI to create IAM roles with the required permissions.

    Once you have run `sam deploy -guided` mode and saved arguments to a configuration file (samconfig.toml), you can use `sam deploy` in future to use these defaults.

6. Note the outputs from the SAM deployment process. These contain the resource names and/or ARNs used for testing.

## How to test

Once deployed, you should have:

* 1 Custom domain -  configured with two paths/one/ and /two/ that are pointing to the relative APIGW
* 2 APIGW one-api and two-api with only GET method pointing to Lambda Function
* 3 Lambda functions - JWT to lookup into DynamoDB and the handler associated to one-api and two-api

**ASSUMPTION:**
1. DynamoDB Scope table is present
2. Custom domain certificate is present
3. Create a Route 53 alias record that routes traffic to the custom domain

Call the custom domain, passing your JWT token in the Authorization header if everything is in place.
https://{route_53_record}/one/

## Cleanup

You can run either:
```
make delete
```
Or:
```
aws cloudformation delete-stack --stack-name STACK_NAME
```

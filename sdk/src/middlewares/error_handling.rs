use crate::error::{ApiError, Error};
use async_trait::async_trait;
use reqwest::{Request, Response};
use reqwest_middleware::{Middleware, Next};
use std::collections::HashMap;
use task_local_extensions::Extensions;

/// Reqwest middleware which translates JSON error responses returned from TrueLayer APIs
/// into [`Error::ApiError`](crate::error::Error)s.
pub struct ErrorHandlingMiddleware;

#[async_trait]
impl Middleware for ErrorHandlingMiddleware {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        // Capture the response
        let response = next.run(req, extensions).await?;

        // Build an error if the response is not a success.
        // Try parsing the contents of the error as an `ErrorResponse`,
        // but if that doesn't work, use the entire contents of the response as the error text.
        if !response.status().is_success() {
            let status = response.status();
            let bytes = response.bytes().await?;

            tracing::debug!("Failed HTTP request. Status code: {}", status);

            let error_response: ErrorResponse =
                serde_json::from_slice(&bytes).unwrap_or_else(|_| ErrorResponse::V1ErrorResponse {
                    error: if bytes.is_empty() {
                        status
                            .canonical_reason()
                            .unwrap_or("Unknown Error")
                            .to_string()
                    } else {
                        String::from_utf8_lossy(&bytes).into_owned()
                    },
                    error_description: None,
                    error_details: None,
                });

            return Err(Error::ApiError(error_response.into_api_error(status.as_u16())).into());
        }

        Ok(response)
    }
}

/// Error response from TrueLayer APIs.
#[derive(serde::Deserialize, Debug)]
#[serde(untagged)]
enum ErrorResponse {
    V3ErrorResponse {
        r#type: String,
        title: String,
        trace_id: String,
        detail: String,
        errors: Option<HashMap<String, Vec<String>>>,
    },
    V1ErrorResponse {
        error: String,
        error_description: Option<String>,
        error_details: Option<HashMap<String, String>>,
    },
}

impl ErrorResponse {
    fn into_api_error(self, http_status: u16) -> ApiError {
        match self {
            ErrorResponse::V3ErrorResponse {
                r#type,
                title,
                trace_id,
                detail,
                errors,
            } => ApiError {
                r#type: Some(r#type),
                title,
                status: http_status,
                trace_id: Some(trace_id),
                detail: Some(detail),
                errors,
            },
            ErrorResponse::V1ErrorResponse {
                error,
                error_description,
                error_details,
            } => ApiError {
                r#type: None,
                title: error,
                status: http_status,
                trace_id: None,
                detail: error_description,
                errors: error_details
                    .map(|errors| errors.into_iter().map(|(k, v)| (k, vec![v])).collect()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn success_responses_are_ignored() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("success"))
            .mount(&mock_server)
            .await;

        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(ErrorHandlingMiddleware)
            .build();

        assert_eq!(
            "success",
            client
                .get(mock_server.uri())
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn json_errors_v1_are_mapped_correctly() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "error": "error",
                "error_description": "description",
                "error_details": {
                    "reason": "yes"
                }
            })))
            .mount(&mock_server)
            .await;

        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(ErrorHandlingMiddleware)
            .build();

        let err: Error = client
            .get(mock_server.uri())
            .send()
            .await
            .expect_err("Call succeeded")
            .into();

        let api_error = match err {
            Error::ApiError(api_error) => api_error,
            e => panic!("Unexpected error: {}", e),
        };

        assert_eq!(api_error.status, 400);
        assert_eq!(api_error.r#type, None);
        assert_eq!(api_error.title, "error");
        assert_eq!(api_error.detail.as_deref(), Some("description"));
        assert_eq!(
            api_error.errors,
            Some(
                [("reason".to_string(), vec!["yes".to_string()])]
                    .into_iter()
                    .collect()
            )
        );
        assert_eq!(api_error.trace_id, None);
    }

    #[tokio::test]
    async fn json_errors_v3_are_mapped_correctly() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(400).set_body_json(json!({
                "type": "https://docs.truelayer.com/docs/error-types#invalid-parameters",
                "title": "Invalid Parameters",
                "status": 400,
                "trace_id": "trace-id",
                "detail": "Some more details",
                "errors": {
                    "reason": [ "one", "two" ]
                }
            })))
            .mount(&mock_server)
            .await;

        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(ErrorHandlingMiddleware)
            .build();

        let err: Error = client
            .get(mock_server.uri())
            .send()
            .await
            .expect_err("Call succeeded")
            .into();

        let api_error = match err {
            Error::ApiError(api_error) => api_error,
            e => panic!("Unexpected error: {}", e),
        };

        assert_eq!(api_error.status, 400);
        assert_eq!(
            api_error.r#type,
            Some("https://docs.truelayer.com/docs/error-types#invalid-parameters".to_string())
        );
        assert_eq!(api_error.title, "Invalid Parameters");
        assert_eq!(api_error.detail.as_deref(), Some("Some more details"));
        assert_eq!(
            api_error.errors,
            Some(
                [(
                    "reason".to_string(),
                    vec!["one".to_string(), "two".to_string()]
                )]
                .into_iter()
                .collect()
            )
        );
        assert_eq!(api_error.trace_id, Some("trace-id".to_string()));
    }

    #[tokio::test]
    async fn non_conforming_json_errors_are_treated_as_text() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(400).set_body_string("non-conforming error text"))
            .mount(&mock_server)
            .await;

        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(ErrorHandlingMiddleware)
            .build();

        let err: Error = client
            .get(mock_server.uri())
            .send()
            .await
            .expect_err("Call succeeded")
            .into();

        let api_error = match err {
            Error::ApiError(api_error) => api_error,
            e => panic!("Unexpected error: {}", e),
        };

        assert_eq!(api_error.status, 400);
        assert_eq!(api_error.r#type, None);
        assert_eq!(api_error.title, "non-conforming error text");
        assert_eq!(api_error.detail.as_deref(), None);
        assert_eq!(api_error.errors, None);
        assert_eq!(api_error.trace_id, None);
    }
}

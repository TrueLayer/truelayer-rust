use crate::{
    common::TL_CORRELATION_ID_HEADER,
    error::{ApiError, Error},
};
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

        // Build an ApiError if the response is not a success
        if !response.status().is_success() {
            tracing::debug!("Failed HTTP request. Status code: {}", response.status());

            let api_error = api_error_from_response(response).await?;
            return Err(Error::ApiError(api_error).into());
        }

        Ok(response)
    }
}

/// Body of an error response from TrueLayer APIs.
#[derive(serde::Deserialize, Debug)]
#[serde(untagged)]
enum ErrorResponseBody {
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
    Unknown,
}

async fn api_error_from_response(response: Response) -> reqwest_middleware::Result<ApiError> {
    let status = response.status().as_u16();
    let tl_correlation_id = response
        .headers()
        .get(TL_CORRELATION_ID_HEADER)
        .map(|v| v.to_str().ok())
        .flatten()
        .map(|v| v.to_string());

    // Parse the response body as JSON
    let bytes = response.bytes().await?;
    let error_response: ErrorResponseBody =
        serde_json::from_slice(&bytes).unwrap_or(ErrorResponseBody::Unknown);

    // Map the legacy error versions
    let api_error = match error_response {
        ErrorResponseBody::V3ErrorResponse {
            r#type,
            title,
            trace_id,
            detail,
            errors,
        } => ApiError {
            r#type,
            title,
            status,
            trace_id: Some(trace_id),
            detail: Some(detail),
            errors: errors.unwrap_or_default(),
        },
        ErrorResponseBody::V1ErrorResponse {
            error,
            error_description,
            error_details,
        } => ApiError {
            r#type: "https://docs.truelayer.com/docs/error-types".to_string(),
            title: error,
            status,
            trace_id: tl_correlation_id,
            detail: error_description,
            errors: error_details
                .map(|errors| errors.into_iter().map(|(k, v)| (k, vec![v])).collect())
                .unwrap_or_default(),
        },
        ErrorResponseBody::Unknown => ApiError {
            r#type: "https://docs.truelayer.com/docs/error-types".to_string(),
            title: "server_error".to_string(),
            status,
            trace_id: tl_correlation_id,
            detail: None,
            errors: Default::default(),
        },
    };

    Ok(api_error)
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
            .respond_with(
                ResponseTemplate::new(400)
                    .append_header(TL_CORRELATION_ID_HEADER, "correlation-id")
                    .set_body_json(json!({
                        "error": "error",
                        "error_description": "description",
                        "error_details": {
                            "reason": "yes"
                        }
                    })),
            )
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
            "https://docs.truelayer.com/docs/error-types"
        );
        assert_eq!(api_error.title, "error");
        assert_eq!(api_error.detail.as_deref(), Some("description"));
        assert_eq!(
            api_error.errors,
            [("reason".to_string(), vec!["yes".to_string()])]
                .into_iter()
                .collect()
        );
        assert_eq!(api_error.trace_id.as_deref(), Some("correlation-id"));
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
            "https://docs.truelayer.com/docs/error-types#invalid-parameters"
        );
        assert_eq!(api_error.title, "Invalid Parameters");
        assert_eq!(api_error.detail.as_deref(), Some("Some more details"));
        assert_eq!(
            api_error.errors,
            [(
                "reason".to_string(),
                vec!["one".to_string(), "two".to_string()]
            )]
            .into_iter()
            .collect()
        );
        assert_eq!(api_error.trace_id, Some("trace-id".to_string()));
    }

    #[tokio::test]
    async fn non_conforming_json_errors_default_to_generic_message() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(400)
                    .append_header(TL_CORRELATION_ID_HEADER, "correlation-id")
                    .set_body_string("non-conforming error text"),
            )
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
            "https://docs.truelayer.com/docs/error-types"
        );
        assert_eq!(api_error.title, "server_error");
        assert_eq!(api_error.detail, None);
        assert_eq!(api_error.errors, HashMap::new());
        assert_eq!(api_error.trace_id.as_deref(), Some("correlation-id"));
    }
}

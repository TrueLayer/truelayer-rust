use crate::error::Error;
use async_trait::async_trait;
use reqwest::{header::HeaderValue, Method, Request, Response};
use reqwest_middleware::{Middleware, Next};
use task_local_extensions::Extensions;

static IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";
static TL_SIGNATURE_HEADER: &str = "Tl-Signature";

/// Middleware to attach signatures to all outgoing `POST`, `PUT` and `DELETE` requests.
///
/// Uses [`truelayer_signing`](truelayer_signing) to build the signatures.
pub struct SigningMiddleware {
    pub(crate) certificate_id: String,
    pub(crate) certificate_private_key: Vec<u8>,
}

#[async_trait]
impl Middleware for SigningMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        // Sign only POST, PUT and DELETE requests
        if let Method::POST | Method::PUT | Method::DELETE = *req.method() {
            // Include method and path
            let mut signer = truelayer_signing::sign_with_pem(
                &self.certificate_id,
                &self.certificate_private_key,
            )
            .method(req.method().as_str())
            .path(req.url().path());

            // Include the idempotency key header
            if let Some(idempotency_key) = req.headers().get(IDEMPOTENCY_KEY_HEADER) {
                signer = signer.header(IDEMPOTENCY_KEY_HEADER, idempotency_key.as_bytes());
            }

            // Include the body
            if let Some(body) = req.body() {
                let bytes = body
                    .as_bytes()
                    .ok_or_else(|| anyhow::anyhow!("Cannot sign a streaming request body"))?;
                signer = signer.body(bytes);
            }

            // Build and attach the signature
            let signature = signer.sign().map_err(Error::from)?;
            let header_value = HeaderValue::from_str(&signature)
                .map_err(|e| reqwest_middleware::Error::Middleware(e.into()))?;
            req.headers_mut().insert(TL_SIGNATURE_HEADER, header_value);
        }

        next.run(req, extensions).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::{
        ec::{EcGroup, EcKey},
        nid::Nid,
        pkey::Private,
    };
    use reqwest_middleware::ClientWithMiddleware;
    use std::str::FromStr;
    use wiremock::{http::HeaderName, matchers::path, Mock, MockServer, ResponseTemplate};

    fn mock_client() -> (ClientWithMiddleware, EcKey<Private>) {
        // Generate a new EC private key
        let key = EcKey::generate(&EcGroup::from_curve_name(Nid::SECP521R1).unwrap()).unwrap();

        let client = reqwest_middleware::ClientBuilder::new(reqwest::Client::new())
            .with(SigningMiddleware {
                certificate_id: "mock-certificate-id".to_string(),
                certificate_private_key: key.private_key_to_pem().unwrap(),
            })
            .build();

        (client, key)
    }

    #[tokio::test]
    async fn includes_signature_only_in_post_put_delete() {
        // Prepare the mock server to capture the requests
        let mock_server = MockServer::start().await;
        Mock::given(path("/test"))
            .respond_with(|req: &wiremock::Request| {
                // Echo back the value of the signature header in the response body
                ResponseTemplate::new(200).set_body_string(
                    req.headers
                        .get(&HeaderName::from_str(TL_SIGNATURE_HEADER).unwrap())
                        .map(|v| v.last().to_string())
                        .unwrap_or_default(),
                )
            })
            .mount(&mock_server)
            .await;

        let table = [
            (Method::GET, false),
            (Method::POST, true),
            (Method::PUT, true),
            (Method::DELETE, true),
        ];

        // Send a test request for all the method names
        let (client, key) = mock_client();
        for (method, expected_signature) in table {
            let idempotency_key = format!("idempotency-key-value-{}", method.as_str());

            let signature = client
                .request(method.clone(), format!("{}/test", mock_server.uri()))
                .header(IDEMPOTENCY_KEY_HEADER, &idempotency_key)
                .body("request-body")
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            assert_eq!(
                !signature.is_empty(),
                expected_signature,
                "Method: {}",
                method
            );

            // Verify the signature
            if expected_signature {
                truelayer_signing::verify_with_pem(key.public_key_to_pem().unwrap().as_slice())
                    .method(method.as_str())
                    .path("/test")
                    .header(IDEMPOTENCY_KEY_HEADER, idempotency_key.as_bytes())
                    .body("request-body".as_bytes())
                    .verify(&signature)
                    .unwrap();
            }
        }
    }
}

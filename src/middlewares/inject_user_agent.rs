use async_trait::async_trait;
use reqwest::{
    header::{HeaderValue, USER_AGENT},
    Request, Response,
};
use reqwest_middleware::{Middleware, Next};
use task_local_extensions::Extensions;

/// Middleware to inject the `User-Agent` header to all outgoing requests.
pub struct InjectUserAgentMiddleware {
    user_agent: HeaderValue,
}

impl InjectUserAgentMiddleware {
    pub fn new() -> Self {
        Self {
            user_agent: concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"))
                .parse()
                .unwrap(),
        }
    }
}

#[async_trait]
impl Middleware for InjectUserAgentMiddleware {
    async fn handle(
        &self,
        mut req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> reqwest_middleware::Result<Response> {
        req.headers_mut()
            .insert(USER_AGENT, self.user_agent.clone());

        next.run(req, extensions).await
    }
}

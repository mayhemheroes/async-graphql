use actix_web::dev::{HttpResponseBuilder, Payload, PayloadStream};
use actix_web::http::StatusCode;
use actix_web::{http, web, Error, FromRequest, HttpRequest, HttpResponse, Responder};
use async_graphql::http::MultipartOptions;
use async_graphql::ParseRequestError;
use futures::channel::mpsc;
use futures::future::Ready;
use futures::io::ErrorKind;
use futures::{Future, SinkExt, StreamExt, TryFutureExt, TryStreamExt};
use http::Method;
use std::io;
use std::pin::Pin;

/// Extractor for GraphQL request
///
/// It's a wrapper of `async_graphql::Request`, you can use `Request::into_inner` unwrap it to `async_graphql::Request`.
/// `async_graphql::http::MultipartOptions` allows to configure extraction process.
pub struct Request(async_graphql::Request);

impl Request {
    /// Unwraps the value to `async_graphql::Request`.
    pub fn into_inner(self) -> async_graphql::Request {
        self.0
    }
}

impl FromRequest for Request {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Request, Error>>>>;
    type Config = MultipartOptions;

    fn from_request(req: &HttpRequest, payload: &mut Payload<PayloadStream>) -> Self::Future {
        let config = req.app_data::<Self::Config>().cloned().unwrap_or_default();

        if req.method() == Method::GET {
            let res = web::Query::<async_graphql::Request>::from_query(req.query_string());
            Box::pin(async move {
                let gql_request = res?;
                Ok(Request(gql_request.into_inner()))
            })
        } else {
            let content_type = req
                .headers()
                .get(http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .map(|value| value.to_string());

            let (mut tx, rx) = mpsc::channel(16);

            // Because Payload is !Send, so forward it to mpsc::Sender
            let mut payload = web::Payload(payload.take());
            actix_rt::spawn(async move {
                while let Some(item) = payload.next().await {
                    if tx.send(item).await.is_err() {
                        return;
                    }
                }
            });

            Box::pin(async move {
                Ok(Request(
                    async_graphql::http::receive_body(
                        content_type,
                        rx.map_err(|err| io::Error::new(ErrorKind::Other, err))
                            .into_async_read(),
                        config,
                    )
                    .map_err(|err| match err {
                        ParseRequestError::PayloadTooLarge => {
                            actix_web::error::ErrorPayloadTooLarge(err)
                        }
                        _ => actix_web::error::ErrorBadRequest(err),
                    })
                    .await?,
                ))
            })
        }
    }
}

/// Responder for GraphQL response
pub struct Response(async_graphql::Response);

impl From<async_graphql::Response> for Response {
    fn from(resp: async_graphql::Response) -> Self {
        Response(resp)
    }
}

impl Responder for Response {
    type Error = Error;
    type Future = Ready<Result<HttpResponse, Error>>;

    fn respond_to(self, _req: &HttpRequest) -> Self::Future {
        let mut res = HttpResponse::build(StatusCode::OK);
        res.content_type("application/json");
        add_cache_control(&mut res, &self.0);
        let res = res.body(serde_json::to_string(&self.0).unwrap());
        futures::future::ok(res)
    }
}

fn add_cache_control(builder: &mut HttpResponseBuilder, resp: &async_graphql::Response) {
    if resp.is_ok() {
        if let Some(cache_control) = resp.cache_control.value() {
            builder.header("cache-control", cache_control);
        }
    }
}